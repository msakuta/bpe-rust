use std::{collections::HashMap, io::Write};

type Elem = u16;

fn main() {
    let file_name = std::env::args()
        .nth(1)
        .unwrap_or("data/island3.cpp".to_string());

    let original_file = std::fs::read_to_string(&file_name)
        .unwrap()
        .into_bytes()
        .iter()
        .map(|b| *b as Elem)
        .collect::<Vec<_>>();
    let mut file = original_file.clone();

    let bpe = encode(&mut file);

    if let Err(e) = write_bpe(
        &file,
        &bpe,
        &mut std::io::BufWriter::new(std::fs::File::create(format!("{file_name}.dat")).unwrap()),
    ) {
        eprintln!("Writing to a file error: {e}");
    }

    decode(&mut file, &bpe);

    println!("file: {}, original: {}", file.len(), original_file.len());

    for (j, (orig, modif)) in original_file.iter().zip(file.iter()).enumerate() {
        assert_eq!(orig, modif, "[{j}]");
    }
    // let bytes = file.iter().filter_map(|b| if *b < 256 { Some(*b as u8)} else { None }).collect::<Vec<_>>();
    // let s = String::from_utf8_lossy(&bytes);
    // println!("{s}");

    assert_eq!(file, original_file);
}

#[derive(Debug)]
struct BpeElem {
    pat: [Elem; 2],
    code: Elem,
    matches: usize,
}

fn encode(file: &mut Vec<Elem>) -> Vec<BpeElem> {
    let mut ret = vec![];
    for i in 0..200 {
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

        let code = ret.len() as Elem + 256;
        println!("[{i}] bp: {}", bp.len());
        println!("[{i}] pb: {:?}", max);

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
            code,
            matches,
        });

        println!("[{i}] file: {} scan time: {elapsed}", file.len());
    }
    ret
}

fn decode(file: &mut Vec<Elem>, bpe: &[BpeElem]) {
    for (i, bp) in bpe.iter().enumerate().rev() {
        let start = std::time::Instant::now();

        let mut matches = 0;
        for j in (0..file.len()).rev() {
            if file[j] == bp.code {
                file[j] = bp.pat[0];
                file.insert(j + 1, bp.pat[1]);
                matches += 1;
            }
        }

        println!("matches: {}, {}", matches, bp.matches);

        let elapsed = start.elapsed().as_secs_f64();

        println!("[{i}] decode: {} scan time: {elapsed}", file.len());
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
        out.write(&bpe_elem.code.to_le_bytes()).unwrap();
    }

    for elem in file.iter() {
        out.write(&elem.to_le_bytes()).unwrap();
    }

    Ok(())
}
