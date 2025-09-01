// async fn dispatcher_tick(cfg: &DispatcherCfg, db: &PgPool) ->
// eyre::Result<()> {     for &chain in &[Chain::ETH, Chain::SOL] {
//         if let Some(n) = next_publish_candidate(chain, db).await? {
//             if publish_ready(cfg, chain, n, db).await? {
//                 enqueue_publish(chain, n, db).await?;
//             }
//         }
//         let back = publish_backlog(chain, db).await?;
//         set_publish_backlog(chain.as_str(), back);
//     }
//     Ok(())
// }
