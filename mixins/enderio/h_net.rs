fn enderio_net(game: &mut crate::game::Game<'_>, player: u128, channel: &str, data: &[u8]) {
    crate::game::enderio_conduits::net_from_client(game, player, channel, data);
}
