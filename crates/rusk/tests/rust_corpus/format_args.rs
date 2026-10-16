use std::fmt::Write;

pub struct Cli {
    pub url: String,
}

pub fn format_args(cli: &Cli, index: usize, name: &str, out: &mut String) -> std::fmt::Result {
    println!("Invalid {}", cli.url);
    let file = format!("{:03}.{name}", index + 1);
    eprintln!("{} {:?} {file}", name, cli.url);
    write!(out, "{}", cli.url)?;
    writeln!(
        out,
        "{} on {}",
        name,
        cli.url.len()
    )?;
    println!("{0} {0}", name);
    println!("{1} {0}", name, cli.url);
    println!("{} {x}", name, x = 1);
    println!("{:>1$}", name, 8);
    println!("{}", "literal");
    println!("{{}} {}", name);
    let shown = format!("{}", (|x: u8| x)(1));
    println!("{shown}");
    Ok(())
}
