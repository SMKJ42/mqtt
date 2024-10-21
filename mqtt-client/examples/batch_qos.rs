use std::process::{Child, Command};

fn main() {
    let programs = vec![
        "pub_qos", "pub_qos1", "pub_qos2", "pub_qos2", "pub_qos2", "pub_qos2", "pub_qos2",
        "pub_qos2", "pub_qos2", "pub_qos2",
    ];

    let mut children: Vec<Child> = vec![];

    let out = Command::new("pwd").output().expect("couldnt pwd");
    println!("{}", String::from_utf8(out.stdout).unwrap());

    for program in programs {
        let child = Command::new("cargo")
            .args(&["run", "--example", program, "--release"])
            .spawn()
            .expect("Failed to start");
        children.push(child);
    }

    println!("All programs completed.");
}
