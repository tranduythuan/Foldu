//! AUTO_PROJECT v2 — tu nhan dien cum du an tu ten file.
//!
//! Khac ban v1 (chi so tien to, nguong cung ">= 2 lan"):
//!   1. Chuan hoa bo dau tieng Viet truoc khi so sanh
//!   2. Loc tu nhieu (v1, IMG, Ban-sao, ngay thang...)
//!   3. Sinh n-gram o MOI vi tri chu khong chi tien to
//!   4. Cham diem co IDF (phat cum qua pho bien) va thuong vi tri dau
//!   5. Nguong dong theo kich thuoc tap file: max(3, ceil(sqrt(n)/2)) * he_so
//!   6. Gop cum gan giong nhau (Jaccard / Levenshtein) — bat loi go tay
//!   7. Giai tan cum < 3 file de chong phan manh vun

use crate::util::{split_name, strip_diacritics};
use std::collections::{HashMap, HashSet};

pub fn other() -> &'static str {
    crate::i18n::t("seg.other")
}

// ------------------------------------------------------------------ Tach token

fn is_date_like(t: &str) -> bool {
    let n = t.len();
    if !(4..=8).contains(&n) || !t.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let year: u32 = t[..4].parse().unwrap_or(0);
    (1900..=2100).contains(&year)
}

fn is_hexish(t: &str) -> bool {
    t.len() >= 8 && t.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn tokenize(filename: &str) -> Vec<String> {
    let (stem, _) = split_name(filename);
    let s = strip_diacritics(&stem);

    // Tach ranh gioi camelCase va ranh gioi chu/so
    let mut spaced = String::with_capacity(s.len() + 8);
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if i > 0 {
            let p = chars[i - 1];
            let boundary = (p.is_lowercase() || p.is_ascii_digit()) && c.is_uppercase()
                || (p.is_alphabetic() && c.is_ascii_digit())
                || (p.is_ascii_digit() && c.is_alphabetic());
            if boundary {
                spaced.push(' ');
            }
        }
        spaced.push(c);
    }

    spaced
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string())
        .collect()
}

fn is_noise(tok: &str, noise: &HashSet<String>) -> bool {
    tok.chars().count() < 2
        || noise.contains(tok)
        || tok.chars().all(|c| c.is_ascii_digit())
        || is_date_like(tok)
        || is_hexish(tok)
}

// ---------------------------------------------------------------- Do tuong dong

fn jaccard(a: &[&str], b: &[&str]) -> f64 {
    let sa: HashSet<&str> = a.iter().copied().collect();
    let sb: HashSet<&str> = b.iter().copied().collect();
    let inter = sa.intersection(&sb).count();
    let uni = sa.len() + sb.len() - inter;
    if uni == 0 {
        0.0
    } else {
        inter as f64 / uni as f64
    }
}

fn levenshtein(a: &str, b: &str) -> usize {
    if a == b {
        return 0;
    }
    let av: Vec<char> = a.chars().collect();
    let bv: Vec<char> = b.chars().collect();
    if av.is_empty() {
        return bv.len();
    }
    if bv.is_empty() {
        return av.len();
    }
    let mut prev: Vec<usize> = (0..=bv.len()).collect();
    let mut cur = vec![0usize; bv.len() + 1];
    for i in 1..=av.len() {
        cur[0] = i;
        for j in 1..=bv.len() {
            let cost = if av[i - 1] == bv[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[bv.len()]
}

// ------------------------------------------------------------------- Thuat toan

pub struct ClusterInput<'a> {
    pub id: u32,
    pub name: &'a str,
}

#[derive(Debug, Clone)]
struct Cluster {
    gram: String,
    aliases: Vec<String>,
    score: f64,
}

/// Tra ve: id file -> ten thu muc cum (hoac `_Khac`)
pub fn cluster_projects(
    files: &[ClusterInput],
    noise_words: &[String],
    granularity: u32,
    max_tokens: usize,
    max_folders: usize,
) -> HashMap<u32, String> {
    let mut result: HashMap<u32, String> = HashMap::new();
    let n = files.len();
    if n < 4 {
        for f in files {
            result.insert(f.id, other().to_string());
        }
        return result;
    }

    let noise: HashSet<String> = noise_words
        .iter()
        .map(|w| strip_diacritics(w).to_lowercase())
        .collect();
    let max_tokens = max_tokens.clamp(1, 6);

    // --- Buoc 1-2: token hoa + loc nhieu, giu ban goc de dat ten dep
    struct Doc {
        id: u32,
        toks: Vec<String>,
        original: String,
    }
    let docs: Vec<Doc> = files
        .iter()
        .map(|f| Doc {
            id: f.id,
            toks: tokenize(f.name)
                .into_iter()
                .filter(|t| !is_noise(t, &noise))
                .collect(),
            original: split_name(f.name).0,
        })
        .collect();

    // --- Buoc 3-4: sinh n-gram + dem tan suat
    let mut count: HashMap<String, usize> = HashMap::new();
    let mut first_pos: HashMap<String, usize> = HashMap::new();
    let mut display: HashMap<String, String> = HashMap::new();

    for d in &docs {
        let mut seen: HashSet<String> = HashSet::new();
        for i in 0..d.toks.len() {
            for len in 1..=max_tokens.min(d.toks.len() - i) {
                let gram = d.toks[i..i + len].join(" ");
                if !seen.insert(gram.clone()) {
                    continue;
                }
                *count.entry(gram.clone()).or_insert(0) += 1;
                if i == 0 {
                    *first_pos.entry(gram.clone()).or_insert(0) += 1;
                }
                display
                    .entry(gram.clone())
                    .or_insert_with(|| pretty_from(&d.original, &gram));
            }
        }
    }

    // --- Buoc 5: nguong dong, dieu chinh boi thanh truot "do min"
    //     granularity 0   -> gop nhieu  -> nguong cao
    //     granularity 100 -> chia nho   -> nguong thap
    let base = ((n as f64).sqrt() / 2.0).ceil().max(3.0);
    let factor = 2.0 - (granularity as f64 / 100.0) * 1.6; // 2.0 .. 0.4
    let min_count = ((base * factor).round() as usize).max(2);

    let mut candidates: Vec<(String, f64)> = Vec::new();
    for (gram, &c) in &count {
        if c < min_count {
            continue;
        }
        let df = c as f64 / n as f64;
        if df > 0.6 {
            continue; // cum xuat hien o >60% file -> gan nhu vo nghia
        }
        let tok_len = gram.split(' ').count() as f64;
        let idf = (1.0 / df.max(0.01)).ln();
        let pos_ratio = *first_pos.get(gram).unwrap_or(&0) as f64 / c as f64;
        let pos_bonus = if pos_ratio > 0.5 { 1.5 } else { 1.0 };
        let score = c as f64 * (1.0 + tok_len).log2() * idf * pos_bonus;
        candidates.push((gram.clone(), score));
    }
    if candidates.is_empty() {
        for f in files {
            result.insert(f.id, other().to_string());
        }
        return result;
    }
    candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // --- Buoc 6: gop cum gan giong nhau
    let mut merged: Vec<Cluster> = Vec::new();
    for (gram, score) in candidates {
        let c_toks: Vec<&str> = gram.split(' ').collect();
        let mut host: Option<usize> = None;
        for (idx, m) in merged.iter().enumerate() {
            let m_toks: Vec<&str> = m.gram.split(' ').collect();
            if jaccard(&c_toks, &m_toks) >= 0.7 {
                host = Some(idx);
                break;
            }
            let max_len = gram.chars().count().max(m.gram.chars().count());
            if max_len > 0 && levenshtein(&gram, &m.gram) as f64 / max_len as f64 <= 0.15 {
                host = Some(idx);
                break;
            }
        }
        match host {
            Some(i) => merged[i].aliases.push(gram),
            None => merged.push(Cluster {
                aliases: vec![gram.clone()],
                gram,
                score,
            }),
        }
    }

    merged.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    merged.truncate(max_folders);

    // --- Buoc 7: gan file vao cum diem cao nhat ma no chua
    let mut gram_to_cluster: HashMap<&str, usize> = HashMap::new();
    for (i, m) in merged.iter().enumerate() {
        for a in &m.aliases {
            gram_to_cluster.insert(a.as_str(), i);
        }
    }

    let mut assign: HashMap<u32, usize> = HashMap::new();
    for d in &docs {
        let mut best: Option<usize> = None;
        let mut best_score = f64::MIN;
        let mut seen: HashSet<String> = HashSet::new();
        for i in 0..d.toks.len() {
            for len in 1..=max_tokens.min(d.toks.len() - i) {
                let gram = d.toks[i..i + len].join(" ");
                if !seen.insert(gram.clone()) {
                    continue;
                }
                if let Some(&ci) = gram_to_cluster.get(gram.as_str()) {
                    if merged[ci].score > best_score {
                        best_score = merged[ci].score;
                        best = Some(ci);
                    }
                }
            }
        }
        if let Some(ci) = best {
            assign.insert(d.id, ci);
        }
    }

    // Giai tan cum qua nho (< 3 file) de chong phan manh vun
    let mut size_of: HashMap<usize, usize> = HashMap::new();
    for &ci in assign.values() {
        *size_of.entry(ci).or_insert(0) += 1;
    }

    for d in &docs {
        let folder = match assign.get(&d.id) {
            Some(&ci) if *size_of.get(&ci).unwrap_or(&0) >= 3 => display
                .get(&merged[ci].gram)
                .cloned()
                .unwrap_or_else(|| merged[ci].gram.clone()),
            _ => other().to_string(),
        };
        result.insert(d.id, folder);
    }
    result
}

/// Lay lai dang co dau tu ten file goc cho cum da bo dau.
/// "Báo cáo tháng 10.pdf" + gram "bao cao thang" -> "Báo-cáo-tháng"
fn pretty_from(original_stem: &str, gram: &str) -> String {
    let g_tok: Vec<&str> = gram.split(' ').collect();
    let o_tok: Vec<&str> = original_stem
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .collect();
    let o_norm: Vec<String> = o_tok
        .iter()
        .map(|t| strip_diacritics(t).to_lowercase())
        .collect();

    if o_tok.len() >= g_tok.len() {
        for i in 0..=(o_tok.len() - g_tok.len()) {
            if (0..g_tok.len()).all(|j| o_norm[i + j] == g_tok[j]) {
                return o_tok[i..i + g_tok.len()].join("-");
            }
        }
    }
    // Khong khop chinh xac (do tach camelCase) -> viet hoa chu cai dau
    g_tok
        .iter()
        .map(|t| {
            let mut ch = t.chars();
            match ch.next() {
                Some(f) => f.to_uppercase().collect::<String>() + ch.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::default_noise;

    #[test]
    fn tokenize_vietnamese() {
        assert_eq!(
            tokenize("Báo-Cáo-Tháng-10.pdf"),
            vec!["bao", "cao", "thang", "10"]
        );
        assert_eq!(tokenize("BaoCaoThang.docx"), vec!["bao", "cao", "thang"]);
    }

    #[test]
    fn noise_filtering() {
        let noise: HashSet<String> = default_noise().into_iter().collect();
        assert!(is_noise("v2", &noise));
        assert!(is_noise("img", &noise));
        assert!(is_noise("20260315", &noise));
        assert!(is_noise("123", &noise));
        assert!(!is_noise("hopdong", &noise));
    }

    #[test]
    fn clusters_vietnamese_project() {
        let _g = crate::i18n::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        crate::i18n::set_lang(crate::i18n::Lang::Vi);

        let names: Vec<String> = (1..=8)
            .map(|i| format!("Báo cáo tháng 10 - phần {}.pdf", i))
            .chain((1..=6).map(|i| format!("Hợp đồng thuê văn phòng v{}.docx", i)))
            .collect();
        let inputs: Vec<ClusterInput> = names
            .iter()
            .enumerate()
            .map(|(i, n)| ClusterInput {
                id: i as u32,
                name: n,
            })
            .collect();
        let out = cluster_projects(&inputs, &default_noise(), 50, 5, 500);

        let first = out.get(&0).unwrap();
        let last = out.get(&13).unwrap();
        assert_ne!(first.as_str(), other(), "nhom bao cao phai duoc nhan dien");
        assert_ne!(last.as_str(), other(), "nhom hop dong phai duoc nhan dien");
        assert_ne!(first, last, "hai du an phai nam o hai thu muc khac nhau");
        // 8 file bao cao phai cung mot cum
        for i in 0..8 {
            assert_eq!(out.get(&i).unwrap(), first);
        }
    }

    #[test]
    fn levenshtein_merges_typos() {
        assert!(levenshtein("bao cao thang", "bao cao thang") == 0);
        assert!(levenshtein("baocao", "bao cao") == 1);
    }
}
