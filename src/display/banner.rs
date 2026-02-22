use colored::Colorize;

const BANNER: [&str; 4] = [
    "   / /    ___    ____   ____ ___   ___    ____ ",
    "  / /_   / _ \\  / __/  / __ `__ \\ / _ \\  /_  /",
    " / __ \\ /  __/ / /    / / / / / //  __/   / / ",
    "/_/ /_/ \\___   /_/    /_/ /_/ /_/\\___    /_/  ",
];

pub fn print_banner() {
    for line in BANNER {
        println!("{}", line.bold().truecolor(159, 43, 104));
    }
}
