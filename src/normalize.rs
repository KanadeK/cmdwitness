use crate::model::NormalizerSpec;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedText {
    pub text: String,
    pub applied: Vec<String>,
}

pub fn apply(input: &str, normalizers: &[NormalizerSpec]) -> NormalizedText {
    let mut text = input.to_owned();
    let mut applied = Vec::new();

    for normalizer in normalizers {
        let (name, next) = match normalizer {
            NormalizerSpec::Ansi => ("ansi", strip_ansi(&text)),
            NormalizerSpec::LineEndings => (
                "lineEndings",
                text.replace("\r\n", "\n").replace('\r', "\n"),
            ),
            NormalizerSpec::Slashes => ("slashes", text.replace('\\', "/")),
            NormalizerSpec::Literal { name, from, to } => (name.as_str(), text.replace(from, to)),
        };
        if next != text {
            applied.push(name.to_owned());
            text = next;
        }
    }

    NormalizedText { text, applied }
}

fn strip_ansi(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] != 0x1b {
            output.push(bytes[index]);
            index += 1;
            continue;
        }

        index += 1;
        if index == bytes.len() {
            break;
        }
        match bytes[index] {
            b'[' => {
                index += 1;
                while index < bytes.len() {
                    let byte = bytes[index];
                    index += 1;
                    if (0x40..=0x7e).contains(&byte) {
                        break;
                    }
                }
            }
            b']' => {
                index += 1;
                while index < bytes.len() {
                    if bytes[index] == 0x07 {
                        index += 1;
                        break;
                    }
                    if bytes[index] == 0x1b
                        && bytes.get(index + 1).is_some_and(|byte| *byte == b'\\')
                    {
                        index += 2;
                        break;
                    }
                    index += 1;
                }
            }
            byte if byte.is_ascii() => index += 1,
            _ => {}
        }
    }

    String::from_utf8(output).expect("removing ASCII escape sequences preserves UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_declared_normalizers_in_order() {
        let result = apply(
            "root\\build\r\nroot\\result\r",
            &[
                NormalizerSpec::LineEndings,
                NormalizerSpec::Slashes,
                NormalizerSpec::Literal {
                    name: "root".into(),
                    from: "root/".into(),
                    to: "<ROOT>/".into(),
                },
            ],
        );
        assert_eq!(result.text, "<ROOT>/build\n<ROOT>/result\n");
        assert_eq!(result.applied, ["lineEndings", "slashes", "root"]);
    }

    #[test]
    fn removes_csi_and_osc_ansi_sequences_without_damaging_unicode() {
        let input = "\u{1b}[31m失败\u{1b}[0m \u{1b}]0;private title\u{7}ok \u{1b}]8;;https://example.test\u{1b}\\link\u{1b}]8;;\u{1b}\\";
        let result = apply(input, &[NormalizerSpec::Ansi]);
        assert_eq!(result.text, "失败 ok link");
        assert_eq!(result.applied, ["ansi"]);
    }

    #[test]
    fn handles_truncated_escape_sequences_without_panicking() {
        assert_eq!(
            apply("before\u{1b}[31", &[NormalizerSpec::Ansi]).text,
            "before"
        );
        assert_eq!(
            apply("before\u{1b}]title", &[NormalizerSpec::Ansi]).text,
            "before"
        );
    }

    #[test]
    fn reports_only_normalizers_that_changed_text() {
        let result = apply(
            "already stable",
            &[
                NormalizerSpec::Ansi,
                NormalizerSpec::LineEndings,
                NormalizerSpec::Literal {
                    name: "missing".into(),
                    from: "nope".into(),
                    to: "x".into(),
                },
            ],
        );
        assert_eq!(result.text, "already stable");
        assert!(result.applied.is_empty());
    }

    #[test]
    fn literal_replacement_is_exact_not_regex_based() {
        let result = apply(
            "a.b a-b",
            &[NormalizerSpec::Literal {
                name: "dot".into(),
                from: "a.b".into(),
                to: "x".into(),
            }],
        );
        assert_eq!(result.text, "x a-b");
    }
}
