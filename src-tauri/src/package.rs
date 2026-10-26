//! Turns the Typst PDF and the CII XML into a `ZUGFeRD` / Factur-X PDF and checks the result.
//! Both are done by Mustang, which also carries veraPDF (PDF/A) and the EN 16931 / `XRechnung` rule
//! sets, so no PDF/A, XMP or business rule logic lives in this app.

use std::ffi::OsString;
use std::path::Path;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::Serialize;

use crate::invoice::ProfileKind;
use crate::sidecar::{self, Sidecars};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Section {
    Pdf,
    Xml,
    Document,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Level {
    Error,
    Warning,
    Notice,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Issue {
    pub section: Section,
    pub level: Level,
    /// The rule id when the message names one (`BR-CO-15`), otherwise Mustang's numeric type
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationReport {
    /// Only true when Mustang calls the file valid and exits cleanly: the gate trusts nothing else
    pub valid: bool,
    pub issues: Vec<Issue>,
}

impl ValidationReport {
    #[cfg(test)]
    pub fn errors(&self) -> impl Iterator<Item = &Issue> {
        self.issues.iter().filter(|i| i.level == Level::Error)
    }
}

fn mustang(sidecars: &Sidecars, dir: &Path, args: &[&str]) -> Result<sidecar::Output> {
    let mut all: Vec<OsString> = [
        "-Xmx768m",
        "-Djava.awt.headless=true",
        "-Dfile.encoding=UTF-8",
        "-jar",
    ]
    .iter()
    .map(Into::into)
    .collect();
    all.push(sidecars.mustang().into());
    all.extend(args.iter().map(Into::into));
    all.push("--disable-file-logging".into());
    sidecar::run(&sidecars.java(), &all, dir, Duration::from_secs(120))
}

/// Attaches `xml` to `pdf` as Factur-X / `ZUGFeRD` 2 (profile EN 16931 or `XRechnung`) into `out`.
///
/// Mustang rewrites the XMP and sets the attachment relationship, the file must not be touched
/// after this: none of the validators checks the relationship.
pub fn combine(
    sidecars: &Sidecars,
    dir: &Path,
    pdf: &str,
    xml: &str,
    profile: ProfileKind,
    out: &str,
) -> Result<()> {
    let profile = match profile {
        ProfileKind::En16931 => "E",
        ProfileKind::XRechnung => "X",
    };
    let result = mustang(
        sidecars,
        dir,
        &[
            "--action",
            "combine",
            "--source",
            pdf,
            "--source-xml",
            xml,
            "--out",
            out,
            "--format",
            "fx",
            "--version",
            "2",
            "--profile",
            profile,
            "--no-additional-attachments",
        ],
    )?;
    if result.code != Some(0) || !dir.join(out).exists() {
        bail!("combining failed: {}", result.stdout.trim());
    }
    Ok(())
}

/// Validates a PDF (PDF/A, XMP, embedded XML) or a plain XML file.
pub fn validate(sidecars: &Sidecars, dir: &Path, file: &str) -> Result<ValidationReport> {
    let result = mustang(sidecars, dir, &["--action", "validate", "--source", file])?;
    let mut report = parse_report(&result.stdout)
        .with_context(|| format!("no readable report, stderr: {}", result.stderr.trim()))?;
    // a non-zero exit with a "valid" report would be a contradiction, the gate stays shut
    report.valid &= result.code == Some(0);
    Ok(report)
}

/// Reads Mustang's XML report (stdout; the logs go to stderr).
pub(crate) fn parse_report(xml: &str) -> Result<ValidationReport> {
    let doc = roxmltree::Document::parse(xml).context("the report is not XML")?;
    let root = doc.root_element();
    if root.tag_name().name() != "validation" {
        bail!("unexpected report root <{}>", root.tag_name().name());
    }
    let status = root
        .children()
        .find(|n| n.has_tag_name("summary"))
        .and_then(|n| n.attribute("status"))
        .context("the report has no overall status")?;

    let mut issues = Vec::new();
    for node in root.descendants().filter(roxmltree::Node::is_element) {
        let level = match node.tag_name().name() {
            "error" | "exception" => Level::Error,
            "warning" => Level::Warning,
            "notice" => Level::Notice,
            "pdf" => {
                // veraPDF's verdict is the element's own text: `ValidationResult [... isCompliant=…]`
                let text = node.text().unwrap_or_default();
                if text.contains("isCompliant=false") {
                    issues.push(Issue {
                        section: Section::Pdf,
                        level: Level::Error,
                        code: "pdfa".into(),
                        message: format!("not PDF/A compliant: {}", text.trim()),
                    });
                }
                continue;
            }
            _ => continue,
        };
        let section = node
            .ancestors()
            .find_map(|a| match a.tag_name().name() {
                "pdf" => Some(Section::Pdf),
                "xml" => Some(Section::Xml),
                _ => None,
            })
            .unwrap_or(Section::Document);
        let message = node.text().unwrap_or_default().trim().to_owned();
        let code = message
            .split_once("[ID ")
            .and_then(|(_, rest)| rest.split_once(']'))
            .map(|(id, _)| id.to_owned())
            .or_else(|| node.attribute("type").map(|t| format!("type-{t}")))
            .unwrap_or_default();
        issues.push(Issue {
            section,
            level,
            code,
            message,
        });
    }

    Ok(ValidationReport {
        valid: status == "valid",
        issues,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const INVALID: &str = r#"<?xml version="1.0" encoding="UTF-8"?>

<validation filename="t.pdf">
  <pdf>ValidationResult [flavour=3b, isCompliant=true]
    <messages>
      <error type="11">XMP Metadata: ConformanceLevel not found</error>
    </messages>
    <summary status="invalid"/>
  </pdf>
  <xml>
    <messages>
      <error type="4" location="/x">[BR-CO-15]-Total wrong. [ID BR-CO-15] from /xslt/a.xslt)</error>
      <notice type="27" location="/y">[BR-DE-15]-Buyer reference [ID BR-DE-15] from /xslt/b.xslt)</notice>
    </messages>
    <summary status="invalid"/>
  </xml>
  <messages>
    <exception type="17">XML could not be extracted</exception>
  </messages>
  <summary status="invalid"/>
</validation>"#;

    #[test]
    fn reads_levels_sections_and_rule_ids() {
        let report = parse_report(INVALID).unwrap();

        assert!(!report.valid);
        let codes: Vec<_> = report
            .issues
            .iter()
            .map(|i| (i.section, i.level, i.code.as_str()))
            .collect();
        assert!(codes.contains(&(Section::Pdf, Level::Error, "type-11")));
        assert!(codes.contains(&(Section::Xml, Level::Error, "BR-CO-15")));
        assert!(codes.contains(&(Section::Xml, Level::Notice, "BR-DE-15")));
        assert!(codes.contains(&(Section::Document, Level::Error, "type-17")));
    }

    #[test]
    fn a_failed_pdfa_check_is_an_error_even_without_messages() {
        let xml = r#"<validation><pdf>ValidationResult [flavour=3u, isCompliant=false]
            <summary status="invalid"/></pdf><summary status="invalid"/></validation>"#;
        let report = parse_report(xml).unwrap();

        assert!(report.errors().any(|i| i.code == "pdfa"));
    }

    #[test]
    fn only_the_overall_status_makes_a_report_valid() {
        let xml = r#"<validation><pdf>ValidationResult [isCompliant=true]<summary status="valid"/></pdf>
            <summary status="valid"/></validation>"#;

        assert!(parse_report(xml).unwrap().valid);
        assert!(parse_report("<validation><pdf/></validation>").is_err());
        assert!(parse_report("not xml").is_err());
    }
}
