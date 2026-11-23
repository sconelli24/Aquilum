use super::functions;
use super::parser;

pub fn syntax_help() -> String {
    format!(
        "{GRAMMAR}\n\nFunctions: {}\n\n{EXAMPLES}",
        functions::available()
    )
}

pub fn check_query(text: &str) -> Result<(), String> {
    parser::parse(text).map(|_| ())
}

const GRAMMAR: &str = "\
A ```dataview block in a note is evaluated inside the app. Query structure:

  TABLE [WITHOUT ID] [expression [AS \"Column\"], …]
  LIST  [WITHOUT ID] [expression]
  TASK  [WITHOUT ID]

Then, in any order:

  TITLE \"Caption\"            caption above the results
  EVERY 30s                   refresh the block automatically, every 30 seconds (5 minutes, 1 hour)
  FROM \"Folder\" | #tag | [[Note]] | outgoing([[Note]]) | once
  WHERE expression            filter condition
  SORT expression [ASC|DESC]  ordering
  LIMIT number                how many rows to keep
  GROUP BY expression [AS name] collapse rows into groups: a key and a list of rows
  FLATTEN expression [AS name]  give the value a name, a list multiplies the row

Parts are applied in the order they are written: LIMIT before SORT really does truncate
before sorting. The source once yields one empty row, for tables that are not about notes.

File fields: file.link, file.path, file.name, file.folder, file.ctime, file.mtime, file.size,
file.tags. Note fields are any name from the frontmatter or from text like 'Key:: value'.
Task fields (TASK only): text, completed, line.

Dates: date(\"2024-10-07\"), today, now, dur(1 week), date ± duration, date - date.
Periods: day, week, month, season, year, ever, span(start, end); they are used by
bar(period), percent(period), remaining(period), count(dates, period), previous(period).";

const EXAMPLES: &str = "\
Examples:

  TABLE WITHOUT ID link(file.link, title) AS \"Note\", rating AS \"Rating\"
  FROM #book
  WHERE rating >= 4
  SORT rating DESC
  LIMIT 10

  TASK TITLE \"Open tasks\"
  WHERE !completed
  SORT file.mtime DESC

  TABLE WITHOUT ID item.name AS \"Period\", bar(item.span) AS \"Progress\"
  FROM once
  EVERY 30s
  FLATTEN list(
      object(\"name\", \"Day\", \"span\", day),
      object(\"name\", \"Year\", \"span\", year)
  ) AS item";

#[cfg(test)]
mod tests {
    use super::{check_query, syntax_help};

    #[test]
    fn help_lists_the_functions_the_build_actually_has() {
        let help = syntax_help();
        assert!(help.contains("dateformat"));
        assert!(help.contains("EVERY 30s"));
        assert!(help.contains("TASK"));
    }

    #[test]
    fn a_query_is_checked_without_touching_the_vault() {
        assert!(check_query("TASK TITLE \"Дела\" WHERE !completed").is_ok());
        let error = check_query("TASK AS TITLE \"Дела\"").expect_err("parsing failed");
        assert!(!error.is_empty());
    }
}
