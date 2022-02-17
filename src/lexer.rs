use anyhow::bail;

#[derive(Debug)]
pub enum Token<'a> {
    Comment(&'a str),
    Param,
    Ident(&'a str),
    Assign,
    Colon,
    SemiColon,
    StringLiteral(&'a str),
    NumberLiteral(&'a str),
    Cell,
    QuestionMark,
    Mul,
    Equal,
    Add,
    Sub,
    ParOpen,
    ParClose,
}

pub fn lex(input: &str) -> Result<Vec<Token>, anyhow::Error> {
    Ok(vec![])
}

#[cfg(test)]
mod tests {
    use super::*;
    use Token::*;

    macro_rules! test {
        ($name:ident,$data:literal, [$($expected_tokens:pat,)*]) => {
            #[test]
            fn $name() {
                let mut tokens = lex($data).unwrap().into_iter();

                $(
                  let token = tokens.next();

                  if !matches!(token, Some($expected_tokens)) {
                      panic!("expected token {:?} found {:?}", stringify!($expected_tokens), token);
                  }
                )*
                let token = tokens.next();
                assert!(token.is_none());
            }
        };
    }

    test! {
        test_lex_comment,
        r#"
# comment 1
   # comment 2
   ### comment
        "#,
        [
            Comment(" comment 1"),
            Comment(" comment 2"),
            Comment("## comment"),
        ]
    }

    test! {
        test_lex_param,
        r#"
param a;
param aa;
param a_b: 1;
param a_b_c : "test";
        "#,
        [
            Param, Ident("a"), SemiColon,
            Param, Ident("aa"), SemiColon,
            Param, Ident("a_b"), Colon, NumberLiteral("1"), SemiColon,
            Param, Ident("a_b_c"), Colon, StringLiteral("test"), SemiColon,
        ]
    }

    test! {
        test_lex_cell,
        r#"
cell cpu_cost:
        total_cpu_core * (
          provider == "gcp"
          ? gcp_cpu_core_cost
          : provider == "aws"
          ? aws_cpu_core_cost
          : azure_cpu_core_cost
        )
;
        "#,
        [
            Cell, Ident("cpu_cost"), Colon,
                Ident("total_cpu_core"), Mul, ParOpen,
                  Ident("provider"), Equal, StringLiteral("gcp"),
                  QuestionMark, Ident("gcp_cpu_core_cost"),
                  Colon, Ident("provider"), Equal, StringLiteral("aws"),
                  QuestionMark, Ident("aws_cpu_core_cost"),
                  Colon, Ident("azure_cpu_core_cost"),
                ParClose,
            SemiColon,
        ]
    }

    test! {
        test_lex_string,
        r#"
"a" "bbbb" "\n\nn" "" "t\tt"
        "#,
        [
            StringLiteral("a"),
            StringLiteral("bbbb"),
            StringLiteral("\n\nn"),
            StringLiteral(""),
            StringLiteral("t\tt"),
        ]
    }

    test! {
        test_lex_number,
        r#"
1
0
9
11
32
1.0
13.50
-13.50
- 13.50
+13.50
+ 13.50
        "#,
        [
            NumberLiteral("1"),
            NumberLiteral("0"),
            NumberLiteral("9"),
            NumberLiteral("11"),
            NumberLiteral("32"),
            NumberLiteral("1.0"),
            NumberLiteral("13.50"),
            NumberLiteral("-13.50"),
            Sub, NumberLiteral("13.50"),
            NumberLiteral("13.50"),
            Add, NumberLiteral("13.50"),
        ]
    }
}
