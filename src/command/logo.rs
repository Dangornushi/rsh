use crate::{RshError, Status};

pub fn rsh_logo() -> Result<Status, RshError> {
    println!("                                    ...................");
    println!("                             ..,77!                     _7!.");
    println!(" `       `  `  `         .,7!.!       ...1.   `              .7&,   `                   `    `  `");
    println!(
        "      `           `  .(7`    \\     ` .\\   ,,   `     `  `      ( .7..     `     `  `"
    );
    println!("  `       `  `    .?!       ,   `    J  .: ,,    `        `     )    7i,`    `       `  `  `   `");
    println!("    `  `        ,^     `    ]        %, .; .,,     `        `   (       ?1,      `               `");
    println!(" `         `  .^    `      .(  `  ` .:t .: \\ (  `    `  `        [        `?(       `   `   `");
    println!("      `  `  ./     `    `  ,        , -, `.`  t            `     j   `       ?,   `        `  `");
    println!("  `        .^   `     `    ,        ,   .J.   ,.  `   `      `   ,     `       4,     `         `");
    println!(
        "    `         .   `        ,  ` `   ,.7!] r.<?~[        `        ,        `     .i      `"
    );
    println!(" `    `  `  t 1    `   `   ,      ` ,   ?7    `]   `      `   `  ,`  `            4.      `  `");
    println!("            (  5.   `    ` ,        ,. ] . 1   ]     `     `     ,     `   `       (.  `       `");
    println!("   `   `    ,   ?,          ) `  `   %`] ]  3  \\  `    `         ,       `   `  `   1.     `     `");
    println!(" `       `   l    5,  `     1        ,.t t  ,./          `   `  `J `  `            .=    `    `");
    println!(
        "    `          i.   ?i,  `  ,,        t  (   (  `   `           .\\        `   ` ..Z."
    );
    println!("      `   `     ,i     .7(,  1  `  `  ,+ . .?         `   `   ` ,    `       .JV");
    println!("                   7+.     ?i,                                 .^      `..JV7^");
    println!("   `                  7(,     ?=..                           :w^  ` ..wV7");
    println!("     `   `   `  `        7..      _71....   `      `  .........(?7&?!                         `  `");
    println!(" `        `       `         ?7(,           _????!!``       ...?7!               `  `   `  `");
    println!("    `  `      `    `             ?7<<... ..       ....(?=`             `  `  `               `");
    Ok(Status::Success)
}
