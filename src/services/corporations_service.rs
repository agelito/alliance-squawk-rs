use crate::{bot::BotNotification, esi::{Esi, EsiID}};
use std::{
    cmp,
    collections::{HashMap, HashSet, VecDeque},
    time::{Duration, Instant},
};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug)]
pub struct CorporationsService {
    esi: Esi,
    alliance_queue: VecDeque<EsiID>,

    alliance_seen: HashSet<EsiID>,
    corporation_alliance: HashMap<EsiID, EsiID>,

    last_alliance_queue_update: Option<Instant>,
    last_alliance_queue_process: Option<Instant>,

    notifications: UnboundedSender<BotNotification>,
}

impl CorporationsService {
    pub fn new(esi: Esi, notifications: UnboundedSender<BotNotification>) -> CorporationsService {
        CorporationsService {
            esi,
            alliance_queue: Default::default(),
            alliance_seen: Default::default(),
            corporation_alliance: Default::default(),
            last_alliance_queue_update: None,
            last_alliance_queue_process: None,
            notifications,
        }
    }

    async fn update_alliance_queue(&mut self) {
        self.last_alliance_queue_update = Some(Instant::now());

        let queue = &mut self.alliance_queue;

        if !queue.is_empty() {
            tracing::warn!(
                "processing queue contains {} items, it will be cleared",
                queue.len()
            );
        }

        queue.clear();

        match self.esi.get_alliance_ids().await {
            Ok(alliance_ids) => {
                for alliance_id in alliance_ids {
                    queue.push_back(alliance_id);
                }
            }
            Err(err) => {
                tracing::error!(?err, "error fetching alliances");
            }
        }

        tracing::info!("queued {} alliances to be processed", queue.len());
    }

    async fn process_alliance_queue(&mut self, limit: Option<usize>) {
        self.last_alliance_queue_process = Some(Instant::now());

        let mut process_limit = if let Some(limit) = limit {
            cmp::min(limit, self.alliance_queue.len())
        } else {
            self.alliance_queue.len()
        };

        if process_limit == 0 {
            tracing::debug!("no alliances queued for processing");
            return;
        }
        
        tracing::info!(
            "processing {} alliances ({} remaining)",
            process_limit,
            self.alliance_queue.len()
        );

        'running: loop {
            if self.alliance_queue.is_empty() || process_limit == 0 {
                break 'running;
            }

            process_limit -= 1;

            let alliance_id = self.alliance_queue.pop_front().expect("queue is not empty");

            tracing::debug!(alliance_id, "updating alliance corporations");

            let mut old_corporations = Vec::new();

            for (c_id, a_id) in self.corporation_alliance.iter() {
                if *a_id == alliance_id {
                    old_corporations.push(*c_id);
                }
            }

            let send_notifications = self.alliance_seen.contains(&alliance_id);

            match self.esi.get_alliance_corporations(alliance_id).await {
                Ok(new_corporations) => {
                    self.alliance_seen.insert(alliance_id);

                    let alliance_ops =
                        corporation_alliance_delta(&old_corporations, &new_corporations);

                    for alliance_op in alliance_ops {
                        match alliance_op {
                            AllianceOp::Add(corporation_id) => {
                                tracing::debug!(
                                    alliance_id,
                                    corporation_id,
                                    "corporation joined alliance"
                                );
                                self.corporation_alliance
                                    .insert(corporation_id, alliance_id);
                            }
                            AllianceOp::Del(corporation_id) => {
                                tracing::debug!(
                                    alliance_id,
                                    corporation_id,
                                    "corporation left alliance"
                                );
                                self.corporation_alliance.remove(&corporation_id);

                                if send_notifications
                                    && self
                                        .notifications
                                        .send(BotNotification::NotifyCorpLeftAlliance(
                                            alliance_id,
                                            corporation_id,
                                        ))
                                        .is_err()
                                {
                                    tracing::warn!(
                                        "aborting service because event channel was closed"
                                    );
                                    break 'running;
                                }
                            }
                        };
                    }
                }
                Err(_) => {
                    tracing::warn!(alliance_id, "couldn't fetch corporations for alliance");
                }
            }
        }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        self.update_alliance_queue().await;
        self.process_alliance_queue(None).await;

        loop {
            match self.last_alliance_queue_update {
                Some(last_alliance_queue_update)
                    if last_alliance_queue_update.elapsed() >= Duration::from_secs(3600 * 2) =>
                {
                    self.update_alliance_queue().await
                }
                _ => {}
            };

            match self.last_alliance_queue_process {
                Some(last_alliance_queue_process)
                    if last_alliance_queue_process.elapsed() >= Duration::from_secs(10) =>
                {
                    self.process_alliance_queue(Some(20)).await
                }
                _ => {}
            };

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

#[derive(Debug, PartialEq)]
enum AllianceOp {
    Add(EsiID),
    Del(EsiID),
}

fn corporation_alliance_delta(
    old_corporations: &Vec<EsiID>,
    new_corporations: &Vec<EsiID>,
) -> Vec<AllianceOp> {
    let mut repetitions = HashMap::new();

    for c in old_corporations {
        if let Some(rep) = repetitions.get(c) {
            repetitions.insert(*c, rep + 1);
        } else {
            repetitions.insert(*c, 1);
        }
    }

    for c in new_corporations {
        if let Some(rep) = repetitions.get(c) {
            repetitions.insert(*c, rep - 1);
        } else {
            repetitions.insert(*c, -1);
        }
    }

    let mut alliance_ops = Vec::new();

    for (corporation_id, repetition) in repetitions {
        match repetition {
            repetition if repetition > 0 => alliance_ops.push(AllianceOp::Del(corporation_id)),
            repetition if repetition < 0 => alliance_ops.push(AllianceOp::Add(corporation_id)),
            _ => {}
        };
    }

    alliance_ops
}

#[cfg(test)]
mod tests {
    use super::{corporation_alliance_delta, AllianceOp};
    use tracing_test::traced_test;

    #[traced_test]
    #[test]
    fn test_corporation_alliance_delta() {
        let old_corporations = vec![0, 1, 2];
        let new_corporations = vec![1, 3];

        let delta = corporation_alliance_delta(&old_corporations, &new_corporations);

        assert!(delta.len() == 3);
    }

    #[traced_test]
    #[test]
    fn test_corporation_alliance_delta_add() {
        let old_corporations = vec![0, 2];
        let new_corporations = vec![0, 1, 2];

        let delta = corporation_alliance_delta(&old_corporations, &new_corporations);

        assert!(delta[0] == AllianceOp::Add(1));
    }

    #[traced_test]
    #[test]
    fn test_corporation_alliance_delta_del() {
        let old_corporations = vec![0, 1, 2];
        let new_corporations = vec![0, 2];

        let delta = corporation_alliance_delta(&old_corporations, &new_corporations);

        assert!(delta[0] == AllianceOp::Del(1));
    }
}
