fn enderio_be(block: &str, entity: &mut Option<(String, horizon_world::nbt::Compound)>) {
    crate::game::enderio::block_entity(block, entity);
}
