//! Parseo/serialización de documentos markdown con front matter YAML opcional
//! (el formato de memorias, skills y comandos de Claude Code).

/// Un documento partido en front matter crudo (sin parsear a tipos) + cuerpo.
///
/// El front matter se guarda como texto crudo, no como struct tipado: así el
/// roundtrip parse -> serialize nunca pierde campos que este crate no conoce
/// (name/description son solo los que nos interesa *leer*, no todo lo que puede
/// tener un frontmatter).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontmatterDoc {
    pub frontmatter_raw: Option<String>,
    pub body: String,
}

impl FrontmatterDoc {
    /// Reconstruye el archivo tal como debería escribirse a disco.
    pub fn to_source(&self) -> String {
        match &self.frontmatter_raw {
            Some(fm) => format!("---\n{}\n---\n{}", fm.trim_end_matches('\n'), self.body),
            None => self.body.clone(),
        }
    }

    /// Busca un campo de primer nivel en el frontmatter (`name: foo` -> `Some("foo")`).
    ///
    /// Deliberadamente NO usa un parser YAML estricto sobre todo el bloque:
    /// varios `argument-hint` reales de este flujo (ej. `[repo] | fix <repo> | heavy`)
    /// no son YAML válido, y con un parser estricto una sola línea así rompía la
    /// lectura de TODOS los campos del documento (incluido `description`). Este
    /// scanner línea por línea es más tosco pero no tiene ese modo de falla.
    pub fn field(&self, key: &str) -> Option<String> {
        let fm = self.frontmatter_raw.as_ref()?;
        top_level_value(fm, key)
    }

    /// Como [`field`](Self::field) pero navegando un nivel anidado
    /// (`["metadata", "type"]` -> el `type` dentro del bloque `metadata:`).
    pub fn field_path(&self, path: &[&str]) -> Option<String> {
        let fm = self.frontmatter_raw.as_ref()?;
        match path {
            [] => None,
            [key] => top_level_value(fm, key),
            [parent, key, ..] => {
                let block = nested_block(fm, parent)?;
                top_level_value(&block, key)
            }
        }
    }
}

/// Busca `key: valor` en las líneas SIN indentar (nivel superior) de `text`.
/// Recorta comillas simples/dobles si el valor viene entre ellas, y entiende
/// el caso `key: >-` / `key: |-` (block scalar YAML multi-línea, lo usa por
/// ejemplo la descripción de `design-reviewer.md`).
fn top_level_value(text: &str, key: &str) -> Option<String> {
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        if line.starts_with(' ') || line.starts_with('\t') {
            continue;
        }
        let Some(after_key) = line.strip_prefix(key) else {
            continue;
        };
        let Some(value) = after_key.strip_prefix(':') else {
            continue;
        };
        let value = value.trim();
        return Some(match block_scalar_fold(value) {
            Some(fold) => collect_block_scalar(&mut lines, fold),
            None => strip_quotes(value),
        });
    }
    None
}

#[derive(Clone, Copy)]
enum BlockFold {
    /// `>` — las líneas se unen con espacios (párrafo).
    Folded,
    /// `|` — las líneas conservan sus saltos de línea.
    Literal,
}

fn block_scalar_fold(value_after_colon: &str) -> Option<BlockFold> {
    match value_after_colon.chars().next()? {
        '>' => Some(BlockFold::Folded),
        '|' => Some(BlockFold::Literal),
        _ => None,
    }
}

/// Junta las líneas indentadas que siguen a un indicador `>`/`|`, hasta volver
/// a nivel 0 (o EOF). No intenta reproducir indicadores de "chomping"
/// (`-`/`+`) al pie de la letra: siempre recorta líneas en blanco finales.
fn collect_block_scalar(
    lines: &mut std::iter::Peekable<std::str::Lines>,
    fold: BlockFold,
) -> String {
    let mut collected = Vec::new();
    while let Some(&line) = lines.peek() {
        if line.trim().is_empty() {
            collected.push(String::new());
            lines.next();
            continue;
        }
        if !(line.starts_with(' ') || line.starts_with('\t')) {
            break;
        }
        collected.push(line.trim().to_string());
        lines.next();
    }
    while collected.last().is_some_and(String::is_empty) {
        collected.pop();
    }
    match fold {
        BlockFold::Folded => collected.join(" "),
        BlockFold::Literal => collected.join("\n"),
    }
}

/// Extrae el sub-bloque indentado debajo de una línea top-level `parent_key:`,
/// des-indentado un nivel (para poder aplicarle `top_level_value` de nuevo).
fn nested_block(text: &str, parent_key: &str) -> Option<String> {
    let mut lines = text.lines();
    let found_parent = lines.by_ref().any(|line| {
        !line.starts_with(' ')
            && !line.starts_with('\t')
            && line.trim_end() == format!("{parent_key}:")
    });
    if !found_parent {
        return None;
    }
    let block: Vec<&str> = lines
        .take_while(|line| line.starts_with(' ') || line.starts_with('\t'))
        .map(str::trim_start)
        .collect();
    if block.is_empty() {
        None
    } else {
        Some(block.join("\n"))
    }
}

fn strip_quotes(value: &str) -> String {
    for quote in ['"', '\''] {
        if value.len() >= 2 && value.starts_with(quote) && value.ends_with(quote) {
            return value[1..value.len() - 1].to_string();
        }
    }
    value.to_string()
}

/// Parsea un documento. Normaliza CRLF -> LF y saca un BOM UTF-8 inicial si
/// hay uno primero: la propiedad que importa no es preservar bytes exactos,
/// sino que `parse(serialize(parse(s)))` sea estable (ver el test de
/// roundtrip). El BOM importa en la práctica: algunos editores lo agregan, y
/// sin sacarlo `strip_prefix("---\n")` nunca matchea y el archivo entero se
/// lee como "sin frontmatter" (le pasaba de verdad a una skill real).
pub fn parse(source: &str) -> FrontmatterDoc {
    let without_bom = source.strip_prefix('\u{feff}').unwrap_or(source);
    // Fast-path: la gran mayoría de los archivos ya vienen con LF; evitamos
    // copiar el contenido entero (el cuerpo de un SKILL.md suele ser extenso)
    // cuando no hay ningún CRLF que normalizar.
    let normalized = if without_bom.contains('\r') {
        without_bom.replace("\r\n", "\n")
    } else {
        without_bom.to_string()
    };
    let Some(rest) = normalized.strip_prefix("---\n") else {
        return FrontmatterDoc {
            frontmatter_raw: None,
            body: normalized,
        };
    };
    match find_closing_delimiter(rest) {
        Some(end_idx) => {
            let frontmatter_raw = rest[..end_idx].trim_end_matches('\n').to_string();
            let after_delim = &rest[end_idx..];
            let body = after_delim
                .strip_prefix("---\n")
                .or_else(|| after_delim.strip_prefix("---"))
                .unwrap_or(after_delim)
                .to_string();
            FrontmatterDoc {
                frontmatter_raw: Some(frontmatter_raw),
                body,
            }
        }
        None => FrontmatterDoc {
            frontmatter_raw: None,
            body: normalized,
        },
    }
}

/// Devuelve el offset (dentro de `rest`) donde arranca la línea de cierre `---`.
fn find_closing_delimiter(rest: &str) -> Option<usize> {
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end_matches('\n') == "---" {
            return Some(offset);
        }
        offset += line.len();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    #[rstest]
    #[case::memoria_con_metadata_anidada(
        "---\nname: rust-ui-research\ndescription: \"Investigación\"\nmetadata:\n  type: project\n---\nCuerpo acá.\n",
        Some("rust-ui-research"),
        Some("Investigación"),
    )]
    #[case::skill_simple(
        "---\nname: ai-failure-logger\ndescription: Automatiza el registro\n---\n# Título\n",
        Some("ai-failure-logger"),
        Some("Automatiza el registro")
    )]
    #[case::comando_con_argument_hint(
        "---\ndescription: Corre el pipeline\nargument-hint: [repo] | fix <repo> | heavy\n---\nContenido.\n",
        None,
        Some("Corre el pipeline"),
    )]
    fn parses_name_and_description(
        #[case] source: &str,
        #[case] expected_name: Option<&str>,
        #[case] expected_description: Option<&str>,
    ) {
        let doc = parse(source);
        assert_eq!(doc.field("name").as_deref(), expected_name);
        assert_eq!(doc.field("description").as_deref(), expected_description);
    }

    #[test]
    fn folded_block_scalar_description_joins_lines_with_spaces() {
        // Caso real: design-reviewer.md usa `description: >-` multi-línea.
        let doc = parse(
            "---\nname: design-reviewer\ndescription: >-\n  Revisa el DISEÑO visual/UX de una pantalla.\n  Es read-only: juzga y prioriza, no edita.\ntools: Bash, Read, Grep, Glob\n---\nBody\n",
        );
        assert_eq!(
            doc.field("description").as_deref(),
            Some("Revisa el DISEÑO visual/UX de una pantalla. Es read-only: juzga y prioriza, no edita.")
        );
        assert_eq!(
            doc.field("tools").as_deref(),
            Some("Bash, Read, Grep, Glob")
        );
    }

    #[test]
    fn literal_block_scalar_preserves_newlines() {
        let doc = parse("---\ndescription: |-\n  linea uno\n  linea dos\n---\nBody\n");
        assert_eq!(
            doc.field("description").as_deref(),
            Some("linea uno\nlinea dos")
        );
    }

    #[test]
    fn leading_utf8_bom_does_not_hide_the_frontmatter() {
        // Bug real: ai-failure-logger/SKILL.md trae un BOM antes del `---` — sin
        // sacarlo, el archivo entero se leía como "sin frontmatter".
        let doc = parse("\u{feff}---\nname: ai-failure-logger\ndescription: Automatiza el registro\n---\nBody\n");
        assert_eq!(doc.field("name").as_deref(), Some("ai-failure-logger"));
        assert_eq!(
            doc.field("description").as_deref(),
            Some("Automatiza el registro")
        );
    }

    #[test]
    fn argument_hint_with_pipes_and_angle_brackets_does_not_break_description() {
        // Este valor real de /audit no es YAML válido (arranca como flow-sequence
        // `[repo]` y sigue con texto suelto) — no debe tumbar la lectura de los
        // demás campos del mismo documento.
        let doc = parse(
            "---\ndescription: Corre el pipeline\nargument-hint: [repo] | fix <repo> | heavy\n---\nContenido.\n",
        );
        assert_eq!(
            doc.field("description").as_deref(),
            Some("Corre el pipeline")
        );
        assert_eq!(
            doc.field("argument-hint").as_deref(),
            Some("[repo] | fix <repo> | heavy")
        );
    }

    #[test]
    fn quoted_argument_hint_strips_the_quotes() {
        let doc = parse("---\ndescription: x\nargument-hint: \"[repo opcional]\"\n---\nBody\n");
        assert_eq!(
            doc.field("argument-hint").as_deref(),
            Some("[repo opcional]")
        );
    }

    #[test]
    fn nested_metadata_type_field() {
        let doc =
            parse("---\nname: x\nmetadata:\n  type: project\n  originSessionId: abc\n---\nBody\n");
        assert_eq!(
            doc.field_path(&["metadata", "type"]).as_deref(),
            Some("project")
        );
    }

    #[test]
    fn missing_frontmatter_keeps_whole_file_as_body() {
        let doc = parse("Solo texto plano, sin frontmatter.\n");
        assert_eq!(doc.frontmatter_raw, None);
        assert_eq!(doc.body, "Solo texto plano, sin frontmatter.\n");
        assert_eq!(doc.field("name"), None);
    }

    #[test]
    fn unterminated_frontmatter_falls_back_to_plain_body() {
        // Sin delimitador de cierre: no es un frontmatter válido, todo es body.
        let source = "---\nname: x\nsin cierre...\n";
        let doc = parse(source);
        assert_eq!(doc.frontmatter_raw, None);
        assert_eq!(doc.body, source);
    }

    #[test]
    fn to_source_roundtrips_a_real_skill_file() {
        let source = "---\nname: ai-failure-logger\ndescription: Automatiza el registro\n---\n# Título\n\nCuerpo.\n";
        let doc = parse(source);
        assert_eq!(doc.to_source(), source);
    }

    #[test]
    fn crlf_is_normalized_before_parsing() {
        let source = "---\r\nname: x\r\n---\r\nCuerpo\r\ncon lineas\r\n";
        let doc = parse(source);
        assert_eq!(doc.field("name").as_deref(), Some("x"));
        assert_eq!(doc.body, "Cuerpo\ncon lineas\n");
    }

    proptest! {
        /// La propiedad que importa de verdad: el ciclo parse -> serialize -> parse
        /// es estable (idempotente), incluso si el input crudo tenía CRLF o
        /// espacios raros que `to_source` no reproduce byte a byte.
        #[test]
        fn parse_serialize_parse_is_idempotent(
            name in "[a-z][a-z0-9-]{0,20}",
            // Sin comillas ni backslash: van embebidas en `description: "..."`
            // sin escapar, así que esos dos caracteres romperían el YAML generado.
            description in "[a-zA-Z0-9 ,.:;!?()_-]{0,60}",
            body in "[ -~\n]{0,200}",
        ) {
            let source = format!("---\nname: {name}\ndescription: \"{description}\"\n---\n{body}");
            let once = parse(&source);
            let twice = parse(&once.to_source());
            prop_assert_eq!(once, twice);
        }
    }
}
