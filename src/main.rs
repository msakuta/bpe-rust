use std::{
    collections::HashMap,
    io::{Read, Write},
    sync::atomic::AtomicBool,
};

type Elem = u16;

static DEBUG: AtomicBool = AtomicBool::new(false);

fn main() {
    let mut file_name = None;
    let mut output_file = None;
    let mut dot = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match &arg as &str {
            "-d" => DEBUG.store(true, std::sync::atomic::Ordering::Release),
            "-o" => output_file = args.next(),
            "-t" => dot = args.next(),
            _ => file_name = Some(arg),
        }
    }

    let Some(file_name) = file_name else {
        let exe = std::env::args().next().unwrap_or_else(|| "exe".to_string());
        println!(
            r#"Usage: {exe} input_file [-d] [-o output_file] [-t dot_file]
        input_file       the file name of an input text file or an encoded file (.dat)
        -d               enable the debug flag, printing verbose information
        -o output_file   specify the name of restored file from encoded file
        -t dot_file      Output dot text for graphviz"#
        );
        return;
    };

    let mut fp = std::io::BufReader::new(std::fs::File::open(&file_name).unwrap());

    let original_file;
    let mut file;
    let bpe;
    if file_name.ends_with(".dat") {
        (file, bpe) = read_bpe(&mut fp).unwrap();
        original_file = None;
    } else {
        let mut tmp_original_file = vec![];
        fp.read_to_end(&mut tmp_original_file).unwrap();

        let tmp_original_file = tmp_original_file
            .iter()
            .map(|b| *b as Elem)
            .collect::<Vec<_>>();
        file = tmp_original_file.clone();
        bpe = encode(&mut file);
        original_file = Some(tmp_original_file);

        if let Err(e) = write_bpe(
            &file,
            &bpe,
            &mut std::io::BufWriter::new(
                std::fs::File::create(format!("{file_name}.dat")).unwrap(),
            ),
        ) {
            eprintln!("Writing to a file error: {e}");
        }
    }

    if let Some(dot) = dot {
        if let Ok(file) = std::fs::File::create(dot) {
            if let Err(e) = write_dot(&bpe, &mut std::io::BufWriter::new(file), true) {
                eprintln!("Error writing dot file: {e}");
            }
        }
    }

    decode(&mut file, &bpe);

    println!("Decoded {} bytes", file.len());

    if let Some(original_file) = original_file {
        println!("file: {}, original: {}", file.len(), original_file.len());

        for (j, (orig, modif)) in original_file.iter().zip(file.iter()).enumerate() {
            assert_eq!(orig, modif, "[{j}]");
        }
        // let bytes = file.iter().filter_map(|b| if *b < 256 { Some(*b as u8)} else { None }).collect::<Vec<_>>();
        // let s = String::from_utf8_lossy(&bytes);
        // println!("{s}");

        assert_eq!(file, original_file);
    }

    if let Some(output_file) = output_file {
        let bytes = file
            .iter()
            .filter_map(|b| if *b < 256 { Some(*b as u8) } else { None })
            .collect::<Vec<_>>();
        std::fs::write(&output_file, bytes).unwrap();
    }
}

/// Byte pair encoding element. An array of this will be the table to translate the encoded data and the decoded one.
/// The code of this element is implicitly decided by adding 256 (representable value by an u8) to the index into
/// the array.
#[derive(Debug)]
struct BpeElem {
    pat: [Elem; 2],
    matches: usize,
}

fn encode(file: &mut Vec<Elem>) -> Vec<BpeElem> {
    let mut ret = vec![];
    for i in 0..10000 {
        let mut bp: HashMap<[Elem; 2], usize> = HashMap::new();

        let start = std::time::Instant::now();

        for (c, cc) in file.iter().zip(file.iter().skip(1)) {
            // println!("{c}{cc}");
            *bp.entry([*c, *cc]).or_default() += 1;
        }

        let elapsed = start.elapsed().as_secs_f64();

        let pb = bp.iter().map(|(k, v)| (v, k)).collect::<Vec<_>>();
        let Some(max) = pb.iter().max_by_key(|(count, _)| **count) else {
            break;
        };

        if *max.0 < 2 {
            break;
        }

        let code = ret.len() as Elem + 256;
        if DEBUG.load(std::sync::atomic::Ordering::Acquire) {
            println!("[{i}] bp: {}", bp.len());
            println!("[{i}] pb: {:?}", max);
        }

        let mut matches = 0;
        for j in (0..file.len() - 1).rev() {
            if &file[j..j + 2] == max.1 {
                file[j] = code;
                file.remove(j + 1);
                matches += 1;
            }
        }

        ret.push(BpeElem {
            pat: *max.1,
            matches,
        });

        if DEBUG.load(std::sync::atomic::Ordering::Acquire) {
            println!("[{i}] file: {} scan time: {elapsed}", file.len());
        }
    }
    ret
}

fn decode(file: &mut Vec<Elem>, bpe: &[BpeElem]) {
    for (i, bp) in bpe.iter().enumerate().rev() {
        // Code is implicitly decided by the number of possible codes in one bytes and the index
        let code = i as Elem + 256;
        let start = std::time::Instant::now();

        let mut matches = 0;
        for j in (0..file.len()).rev() {
            if file[j] == code {
                file[j] = bp.pat[0];
                file.insert(j + 1, bp.pat[1]);
                matches += 1;
            }
        }

        let elapsed = start.elapsed().as_secs_f64();

        if DEBUG.load(std::sync::atomic::Ordering::Acquire) {
            println!(
                "[{i}] decode: {file_len}, matches: {matches}, {bp_matches} scan time: {elapsed}",
                bp_matches = bp.matches,
                file_len = file.len()
            );
        }
    }
}

const SIGNATURE: [u8; 4] = [200, 191, 111, 0];

fn write_bpe(file: &[Elem], bpe: &[BpeElem], out: &mut impl Write) -> std::io::Result<()> {
    out.write_all(&SIGNATURE)?;
    out.write_all(&(bpe.len() as u32).to_le_bytes())?;
    for bpe_elem in bpe {
        for pat in bpe_elem.pat {
            out.write(&pat.to_le_bytes()).unwrap();
        }
    }

    for elem in file.iter() {
        out.write(&elem.to_le_bytes()).unwrap();
    }

    Ok(())
}

fn read_bpe(input: &mut impl Read) -> std::io::Result<(Vec<Elem>, Vec<BpeElem>)> {
    let mut sigbuf = [0u8; 4];
    input.read_exact(&mut sigbuf)?;
    if sigbuf != SIGNATURE {
        return Err(std::io::Error::other("Signature does not match"));
    }

    let mut num_bpe_elems = [0u8; std::mem::size_of::<u32>()];
    input.read_exact(&mut num_bpe_elems)?;
    let num_bpe_elems = u32::from_le_bytes(num_bpe_elems);

    let bpe = (0..num_bpe_elems)
        .map(|_| -> std::io::Result<_> {
            let mut pat = [0; 2];
            for j in 0..2 {
                let mut pat_but = [0u8; std::mem::size_of::<Elem>()];
                input.read_exact(&mut pat_but)?;
                pat[j] = Elem::from_le_bytes(pat_but);
            }
            Ok(BpeElem { pat, matches: 0 })
        })
        .collect::<Result<Vec<_>, _>>()?;

    if DEBUG.load(std::sync::atomic::Ordering::Acquire) {
        println!("BPE loaded {}", bpe.len());
    }

    let mut elem_buf = [0u8; std::mem::size_of::<Elem>()];
    let mut file_buf = vec![];
    while let Ok(size) = input.read(&mut elem_buf) {
        if size != std::mem::size_of::<Elem>() {
            break;
        }
        file_buf.push(Elem::from_le_bytes(elem_buf));
    }

    let file = file_buf.iter().map(|b| *b as Elem).collect();

    Ok((file, bpe))
}

fn write_dot(bpe: &[BpeElem], out: &mut impl Write, horizontal: bool) -> std::io::Result<()> {
    writeln!(out, "digraph {{")?;
    if horizontal {
        writeln!(out, "rankdir=LR")?;
    }
    for (i, bpe_elem) in bpe.iter().enumerate() {
        let code = i as Elem + 256;
        let elems = reconstruct_bpe_elem(bpe, code);
        let label = elems.and_then(|elems| {
            let bytes = elems
                .iter()
                .filter_map(|elem| if *elem < 256 { Some(*elem as u8) } else { None })
                .collect::<Vec<_>>();
            let str = double_quote(&String::from_utf8(bytes).ok()?);
            Some(double_quote(&format!("{} \"{}\"", code, str)))
        });
        if let Some(label) = label {
            writeln!(out, "{} [label=\"{}\"]", code, label)?;
        }
        writeln!(out, "{} -> {}", code, bpe_elem.pat[0])?;
        writeln!(out, "{} -> {}", code, bpe_elem.pat[1])?;
    }
    writeln!(out, "}}")?;
    Ok(())
}

fn reconstruct_bpe_elem(bpe: &[BpeElem], idx: Elem) -> Option<Vec<Elem>> {
    if idx < 256 {
        return Some(vec![idx]);
    }
    let Some(elem) = bpe.get(idx as usize - 256) else {
        return None;
    };
    reconstruct_bpe_elem(bpe, elem.pat[0])
        .zip(reconstruct_bpe_elem(bpe, elem.pat[1]))
        .map(|(mut l, r)| {
            l.extend_from_slice(&r);
            l
        })
}

fn double_quote(str: &str) -> String {
    str.replace("\\", "\\\\")
        .replace("\"", "\\\"")
        .replace("\r\n", "\\n")
        .replace("\n", "\\n")
}
