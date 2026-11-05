use rowan::TextSize;

/// A position as the TradeServer reports it: 1-based line, 1-based column
/// counted in characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineColumn {
    pub line: usize,
    pub column: usize,
}

pub fn line_column(source: &str, offset: TextSize) -> LineColumn {
    let before = &source[..usize::from(offset)];
    let line_start = before.rfind('\n').map_or(0, |index| index + 1);
    LineColumn { line: before.matches('\n').count() + 1, column: before[line_start..].chars().count() + 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_character_is_line_one_column_one() {
        assert_eq!(line_column("abc", TextSize::from(0)), LineColumn { line: 1, column: 1 });
    }

    #[test]
    fn columns_count_characters_not_bytes() {
        let source = "x\n\u{e9}y";
        let offset = TextSize::from(u32::try_from(source.find('y').unwrap()).unwrap());
        assert_eq!(line_column(source, offset), LineColumn { line: 2, column: 2 });
    }
}
