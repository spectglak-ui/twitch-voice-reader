// Empêche l'ouverture d'une console supplémentaire sous Windows en mode release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    twitch_voice_reader_lib::run();
}
