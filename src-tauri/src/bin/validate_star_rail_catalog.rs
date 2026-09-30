fn main() -> Result<(), String> {
    genshin_reco_lib::star_rail::load_star_rail_catalog()?;
    Ok(())
}
