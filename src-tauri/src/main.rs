// Khong hien cua so console den khi chay ban phat hanh tren Windows
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    foldu_lib::run()
}
