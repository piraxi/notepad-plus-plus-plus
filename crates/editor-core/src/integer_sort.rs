use std::cmp::Ordering;

/// Port of Sorters.h IntegerSorter. Numbers have arbitrary length; leading
/// zeros break numeric ties before following text, even for negative numbers.
pub(crate) fn compare(a: &[u16], b: &[u16]) -> Ordering {
    let digit = |unit: u16| (48..=57).contains(&unit);
    let number = |text: &[u16], i: usize| {
        digit(text[i]) || (text[i] == 45 && text.get(i + 1).is_some_and(|&u| digit(u)))
    };
    let (mut i, mut j) = (0, 0);
    loop {
        if i == a.len() || j == b.len() {
            return a[i..].cmp(&b[j..]);
        }
        let (an, bn) = (number(a, i), number(b, j));
        let result = if an != bn {
            let order = a[i].cmp(&b[j]);
            i += 1;
            j += 1;
            order
        } else if an {
            let (negative_a, negative_b) = (a[i] == 45, b[j] == 45);
            if negative_a != negative_b {
                return negative_b.cmp(&negative_a);
            }
            if negative_a {
                i += 1;
                j += 1;
            }
            let a_end = i + a[i..].iter().take_while(|&&u| digit(u)).count();
            let b_end = j + b[j..].iter().take_while(|&&u| digit(u)).count();
            let az = a[i..a_end].iter().take_while(|&&u| u == 48).count();
            let bz = b[j..b_end].iter().take_while(|&&u| u == 48).count();
            i += az;
            j += bz;
            let magnitude = (a_end - i)
                .cmp(&(b_end - j))
                .then_with(|| a[i..a_end].cmp(&b[j..b_end]));
            let order = if negative_a {
                magnitude.reverse()
            } else {
                magnitude
            };
            i = a_end;
            j = b_end;
            order.then_with(|| bz.cmp(&az))
        } else {
            // A minus sign not followed by a digit is skipped in text chunks
            // by the supplied comparator (not treated as a numeric sign).
            if a[i] == 45 {
                i += 1;
            }
            if b[j] == 45 {
                j += 1;
            }
            let a_end = i + a[i..].iter().take_while(|&&u| !digit(u) && u != 45).count();
            let b_end = j + b[j..].iter().take_while(|&&u| !digit(u) && u != 45).count();
            let order = a[i..a_end].cmp(&b[j..b_end]);
            i = a_end;
            j = b_end;
            order
        };
        if result != Ordering::Equal {
            return result;
        }
    }
}
