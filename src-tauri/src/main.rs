// Windowsのリリースビルドで追加のコンソールを表示しない。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    genshin_reco_lib::run()
}
