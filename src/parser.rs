use crate::models::{Side, Signal};
fn extract_number_after(text: &str, keywords: &[&str]) -> Option<f64> {
    for &kw in keywords {
        if let Some(mut idx) = text.find(kw) {
            idx += kw.len();
            let after_kw = &text[idx..];
            
            // Buscar el primer dígito
            if let Some(start_idx) = after_kw.find(|c: char| c.is_ascii_digit()) {
                let nums_str = &after_kw[start_idx..];
                // Encontrar el final del número (permitiendo un punto decimal)
                let end_idx = nums_str.find(|c: char| !c.is_ascii_digit() && c != '.').unwrap_or(nums_str.len());
                if end_idx > 0 {
                    if let Ok(val) = nums_str[..end_idx].parse::<f64>() {
                        return Some(val);
                    }
                }
            }
        }
    }
    None
}

pub fn parse_signal(text: &str, msg_id: i32, received_at: std::time::Instant) -> Option<Signal> {
    let text_upper = text.to_uppercase();
    
    // Quick discard for non-signals or outcome messages
    if text_upper.contains("HIT") || text_upper.contains("%") {
        return None; // Ignore profit hits / results
    }

    if !text_upper.contains("LONG") && !text_upper.contains("SHORT") 
        && !text_upper.contains("BUY") && !text_upper.contains("SELL")
        && !text_upper.contains("BULLISH") && !text_upper.contains("BEARISH") {
        return None;
    }

    // Extract Symbol
    let hash_idx = text_upper.find('#')?;
    let after_hash = &text_upper[hash_idx + 1..];
    let end_idx = after_hash.find(|c: char| !c.is_ascii_alphanumeric()).unwrap_or(after_hash.len());
    if end_idx == 0 { return None; }
    
    let s = &after_hash[..end_idx];
    let symbol = if s.ends_with("USDT") { s.to_string() } else { format!("{}USDT", s) };

    // Extract Side
    let side = if text_upper.contains("LONG") || text_upper.contains("BUY") || text_upper.contains("BULLISH") {
        Side::Long
    } else if text_upper.contains("SHORT") || text_upper.contains("SELL") || text_upper.contains("BEARISH") {
        Side::Short
    } else {
        return None;
    };

    let leverage = extract_number_after(&text_upper, &["LEVERAGE", "APALANCAMIENTO", "APAL", "LEV"])
        .map(|v| v as u32);
    let entry = extract_number_after(&text_upper, &["ENTRY", "ENTRADA", "CMP"]);
    let sl = extract_number_after(&text_upper, &["STOP LOSS", "STOP", "SL"]);
    let tp = extract_number_after(&text_upper, &["TAKE PROFIT", "TARGET", "TP"]);

    Some(Signal {
        msg_id,
        symbol,
        side,
        leverage,
        entry,
        sl,
        tp,
        received_at,
    })
}
