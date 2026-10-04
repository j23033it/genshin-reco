fn main() -> Result<(), String> {
    let catalog = genshin_reco_lib::star_rail::load_star_rail_catalog()?;
    genshin_reco_lib::star_rail::validate_catalog_inventory(&catalog)?;
    Ok(())
}
