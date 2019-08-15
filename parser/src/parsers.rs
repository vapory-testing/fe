use nom::bytes::complete::{tag, take_while1};
use nom::character::complete::{char, multispace0, multispace1, space0, space1};
use nom::combinator::verify;
use nom::error::{context, ParseError};
use nom::multi::many0;
use nom::sequence::{preceded, terminated};
use nom::{Err as NomErr, IResult, InputTake};

use crate::ast::ModuleStmt::*;
use crate::ast::*;

/// Return true if the char `c` is a valid symbol character.
pub fn is_symbol(c: char) -> bool {
    c == '_' || c.is_ascii_alphabetic() || c.is_digit(10)
}

/// Parse a symbol i.e. `_foo`, `Foo`, `foo_Bar_1`, etc.  Symbols may not begin with numbers.
pub fn symbol<'a, E>(inp: &'a str) -> IResult<&'a str, &'a str, E>
where
    E: ParseError<&'a str>,
{
    verify(take_while1(is_symbol), |s: &str| {
        !s.chars().next().unwrap().is_digit(10)
    })(inp)
}

/// Parse a sequence of whitespace characters that must contain at least one newline.  Remaining
/// parser input begins just after the last parsed newline.  Parser output includes all whitespace
/// chars parsed up to the last newline.
pub fn ws_nl<'a, E>(inp: &'a str) -> IResult<&'a str, &'a str, E>
where
    E: ParseError<&'a str>,
{
    let (_, out) = multispace1(inp)?;
    let last_newline_i = out.char_indices().rev().find(|i| i.1 == '\n');

    if let Some((i, _)) = last_newline_i {
        Ok(inp.take_split(i + 1))
    } else {
        Err(NomErr::Error(E::from_char(inp, '\n')))
    }
}

/// Return a parser that requires the specific sequence of indentation characters in `indent` to be
/// present before the parsed content.
pub fn indented<'a, F, O, E>(indent: &'a str, parser: F) -> impl Fn(&'a str) -> IResult<&'a str, O, E>
where
    F: Fn(&'a str) -> IResult<&'a str, O, E>,
    E: ParseError<&'a str>,
{
    preceded(context("indentation", tag(indent)), parser)
}

pub fn parse_module<'a, E>(inp: &'a str) -> IResult<&'a str, Module, E>
where
    E: ParseError<&'a str>,
{
    // (ws module_stmt)*
    let (i, body) = many0(preceded(multispace0, parse_module_stmt))(inp)?;

    Ok((i, Module { body: body }))
}

pub fn parse_module_stmt<'a, E>(inp: &'a str) -> IResult<&'a str, ModuleStmt, E>
where
    E: ParseError<&'a str>,
{
    let (i, module_stmt) = context("event definition", parse_event_def)(inp)?;

    Ok((i, module_stmt))
}

pub fn parse_event_def<'a, E>(inp: &'a str) -> IResult<&'a str, ModuleStmt, E>
where
    E: ParseError<&'a str>,
{
    // "event" symbol ":" ws_nl
    let (i, _) = terminated(tag("event"), space1)(inp)?;
    let (i, name) = terminated(symbol, space0)(i)?;
    let (i, _) = terminated(char(':'), ws_nl)(i)?;

    // Determine indentation level
    let (_, indent) = space1(i)?;
    let ind_event_field = indented(indent, parse_event_field);

    // <indent> event_field (ws_nl event_field)*
    let (i, first_field) = ind_event_field(i)?;
    let (i, mut other_fields) = many0(preceded(ws_nl, ind_event_field))(i)?;

    let mut fields = vec![first_field];
    fields.append(&mut other_fields);

    Ok((
        i,
        EventDef {
            name: name.to_string(),
            fields: fields,
        },
    ))
}

pub fn parse_event_field<'a, E>(inp: &'a str) -> IResult<&'a str, EventField, E>
where
    E: ParseError<&'a str>,
{
    let (i, name) = terminated(symbol, space0)(inp)?;
    let (i, _) = terminated(char(':'), space0)(i)?;
    let (i, typ) = symbol(i)?;

    Ok((
        i,
        EventField {
            name: name.to_string(),
            typ: typ.to_string(),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    use nom::error::{convert_error, ErrorKind, ErrorKind::*, VerboseError};
    use nom::Err as NomErr;

    use crate::errors::make_error;

    type SimpleError<'a> = (&'a str, ErrorKind);

    #[test]
    fn test_symbol() {
        // Success
        let examples = vec![
            ("Foo", Ok(("", "Foo"))),
            ("FooBar_Baz", Ok(("", "FooBar_Baz"))),
            ("_foo", Ok(("", "_foo"))),
            ("foo_bar", Ok(("", "foo_bar"))),
            ("_foo123  ", Ok(("  ", "_foo123"))),
        ];
        for (inp, expected) in examples {
            let actual = symbol::<SimpleError>(inp);
            assert_eq!(actual, expected);
        }

        // Error
        let examples = vec![
            ("  foo", make_error("  foo", TakeWhile1)),
            ("12foo", make_error("12foo", Verify)),
            ("", make_error("", TakeWhile1)),
        ];
        for (inp, expected) in examples {
            let actual = symbol::<SimpleError>(inp);
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn scratch() {
        let s = "asfasdf".to_string();
        let len = s.len();
        println!("{:?}", &s[len - 1..].chars().next().unwrap());
    }

    #[test]
    fn test_ws_nl() {
        // Success
        let examples = vec![
            ("\n", Ok(("", "\n"))),
            ("\n \n", Ok(("", "\n \n"))),
            (" \n    ", Ok(("    ", " \n"))),
            (" \n \n ", Ok((" ", " \n \n"))),
            ("  \n   \n", Ok(("", "  \n   \n"))),
            ("  \n   \n     ", Ok(("     ", "  \n   \n"))),
        ];
        for (inp, expected) in examples {
            let actual = ws_nl::<SimpleError>(inp);
            assert_eq!(actual, expected);
        }

        // Error
        let examples = vec![
            ("", make_error("", MultiSpace)),
            ("  ", make_error("  ", Char)),
            ("  foo", make_error("  foo", Char)),
        ];
        for (inp, expected) in examples {
            let actual = ws_nl::<SimpleError>(inp);
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn test_module() {
        let input = r"
event Greet:
    name: bytes32
    age: uint8";
        let res = parse_module::<VerboseError<&str>>(input);

        assert_eq!(
            res,
            Ok((
                "",
                Module {
                    body: vec![EventDef {
                        name: "Greet".to_string(),
                        fields: vec![
                            EventField {
                                name: "name".to_string(),
                                typ: "bytes32".to_string(),
                            },
                            EventField {
                                name: "age".to_string(),
                                typ: "uint8".to_string(),
                            },
                        ],
                    }]
                }
            )),
        );
    }
}
