//! Port alineado a `OpenTTD`; casts/bucles intencionales.
#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::double_must_use,
    clippy::if_not_else,
    clippy::items_after_statements,
    clippy::mut_range_bound,
    clippy::needless_range_loop,
    clippy::should_implement_trait
)]

use std::collections::BTreeMap;
use std::ops::Bound::{Excluded, Unbounded};

use super::rng::Randomizer;
use super::types::INVALID_STATION;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FlowStat {
    shares: BTreeMap<u32, u32>,
    unrestricted: u32,
}

impl FlowStat {
    #[must_use]
    pub fn new(station: u32, flow: u32, restricted: bool) -> Self {
        let mut shares = BTreeMap::new();
        shares.insert(flow, station);
        Self {
            shares,
            unrestricted: if restricted { 0 } else { flow },
        }
    }

    #[must_use]
    pub fn shares(&self) -> &BTreeMap<u32, u32> {
        &self.shares
    }

    #[must_use]
    pub fn unrestricted(&self) -> u32 {
        self.unrestricted
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.shares.is_empty()
    }

    #[must_use]
    pub fn total_flow(&self) -> u32 {
        self.shares.last_key_value().map_or(0, |(key, _)| *key)
    }

    pub fn append_share(&mut self, station: u32, flow: u32, restricted: bool) {
        if flow == 0 {
            return;
        }
        let end = self.total_flow();
        self.shares.insert(end.saturating_add(flow), station);
        if !restricted {
            self.unrestricted = self.unrestricted.saturating_add(flow);
        }
    }

    #[must_use]
    pub fn get_share(&self, station: u32) -> u32 {
        let mut previous = 0_u32;
        for (share, via_station) in &self.shares {
            if *via_station == station {
                return share.saturating_sub(previous);
            }
            previous = *share;
        }
        0
    }

    #[must_use]
    pub fn get_via(&self, excluded: u32, excluded2: u32, rng: &mut Randomizer) -> u32 {
        if self.unrestricted == 0 || self.shares.is_empty() {
            return INVALID_STATION;
        }

        let Some((end, via)) = self.upper_bound_station(rng.random_range(self.unrestricted)) else {
            return INVALID_STATION;
        };
        if via != excluded && via != excluded2 {
            return via;
        }

        let begin = self.previous_share(end);
        let interval = end.saturating_sub(begin);
        if interval >= self.unrestricted {
            return INVALID_STATION;
        }

        let mut new_max = self.unrestricted - interval;
        let rand = rng.random_range(new_max);
        let lookup = if rand < begin { rand } else { rand + interval };
        let Some((end2, via2)) = self.upper_bound_station(lookup) else {
            return INVALID_STATION;
        };
        if via2 != excluded && via2 != excluded2 {
            return via2;
        }

        let begin2 = self.previous_share(end2);
        let mut interval2 = end2.saturating_sub(begin2);
        if interval2 >= new_max {
            return INVALID_STATION;
        }

        new_max -= interval2;
        let mut begin_a = begin;
        let mut end_a = end;
        let mut interval_a = interval;
        let mut begin_b = begin2;
        if begin_a > begin_b {
            std::mem::swap(&mut begin_a, &mut begin_b);
            std::mem::swap(&mut end_a, &mut interval2);
            std::mem::swap(&mut interval_a, &mut interval2);
        }

        let rand = rng.random_range(new_max);
        let lookup = if rand < begin_a {
            rand
        } else if rand < begin_b.saturating_sub(interval_a) {
            rand + interval_a
        } else {
            rand + interval_a + interval2
        };
        self.upper_bound_station(lookup)
            .map_or(INVALID_STATION, |(_, station)| station)
    }

    pub fn invalidate(&mut self) {
        if self.shares.is_empty() {
            return;
        }
        let mut new_shares = BTreeMap::new();
        let mut next = 0_u32;
        let old_unrestricted = self.unrestricted;
        for (share, station) in &self.shares {
            next = next.saturating_add(1);
            new_shares.insert(next, *station);
            if *share == old_unrestricted {
                self.unrestricted = next;
            }
        }
        self.shares = new_shares;
    }

    pub fn change_share(&mut self, station: u32, flow: i32) {
        if self.shares.is_empty() {
            return;
        }

        let mut removed_shares = 0_u32;
        let mut added_shares = 0_u32;
        let mut last_share = 0_u32;
        let mut new_shares = BTreeMap::new();
        let mut remaining_flow = flow;
        let mut needs_release = false;

        for (share_end, via_station) in &self.shares {
            if *via_station == station {
                if remaining_flow < 0 {
                    let share = share_end.saturating_sub(last_share);
                    let remove_all =
                        remaining_flow == i32::MIN || remaining_flow.unsigned_abs() >= share;
                    if remove_all {
                        removed_shares = removed_shares.saturating_add(share);
                        if *share_end <= self.unrestricted {
                            self.unrestricted = self.unrestricted.saturating_sub(share);
                        }
                        if remaining_flow != i32::MIN {
                            remaining_flow = remaining_flow.saturating_add(share as i32);
                        }
                        last_share = *share_end;
                        continue;
                    }
                    removed_shares = removed_shares.saturating_add(remaining_flow.unsigned_abs());
                    if *share_end <= self.unrestricted {
                        self.unrestricted = self
                            .unrestricted
                            .saturating_sub(remaining_flow.unsigned_abs());
                    }
                } else {
                    let added = remaining_flow as u32;
                    added_shares = added_shares.saturating_add(added);
                    if *share_end <= self.unrestricted {
                        self.unrestricted = self.unrestricted.saturating_add(added);
                    }
                }
                remaining_flow = 0;
            }

            let new_key = share_end
                .saturating_add(added_shares)
                .saturating_sub(removed_shares);
            new_shares.insert(new_key, *via_station);
            last_share = *share_end;
        }

        if remaining_flow > 0 {
            let added = remaining_flow as u32;
            new_shares.insert(last_share.saturating_add(added), station);
            if self.unrestricted < last_share {
                needs_release = true;
            } else {
                self.unrestricted = self.unrestricted.saturating_add(added);
            }
        }

        self.shares = new_shares;
        if needs_release {
            self.release_share(station);
        }
    }

    pub fn restrict_share(&mut self, station: u32) {
        if self.shares.is_empty() {
            return;
        }
        let mut flow = 0_u32;
        let mut last_share = 0_u32;
        let mut new_shares = BTreeMap::new();
        for (share_end, via_station) in &self.shares {
            if flow == 0 {
                if *share_end > self.unrestricted {
                    return;
                }
                if *via_station == station {
                    flow = share_end.saturating_sub(last_share);
                    self.unrestricted = self.unrestricted.saturating_sub(flow);
                } else {
                    new_shares.insert(*share_end, *via_station);
                }
            } else {
                new_shares.insert(share_end.saturating_sub(flow), *via_station);
            }
            last_share = *share_end;
        }
        if flow == 0 {
            return;
        }
        new_shares.insert(last_share.saturating_add(flow), station);
        self.shares = new_shares;
    }

    pub fn release_share(&mut self, station: u32) {
        if self.shares.is_empty() {
            return;
        }

        let mut flow = 0_u32;
        let mut next_share = 0_u32;
        let mut found = false;
        for (share_end, via_station) in self.shares.iter().rev() {
            if *share_end < self.unrestricted {
                return;
            }
            if found {
                flow = next_share.saturating_sub(*share_end);
                self.unrestricted = self.unrestricted.saturating_add(flow);
                break;
            }
            if *share_end == self.unrestricted {
                return;
            }
            if *via_station == station {
                found = true;
            }
            next_share = *share_end;
        }
        if flow == 0 {
            return;
        }

        let mut new_shares = BTreeMap::new();
        new_shares.insert(flow, station);
        for (share_end, via_station) in &self.shares {
            if *via_station != station {
                new_shares.insert(flow.saturating_add(*share_end), *via_station);
            } else {
                flow = 0;
            }
        }
        self.shares = new_shares;
    }

    pub fn scale_to_monthly(&mut self, runtime: u32) {
        if runtime == 0 {
            return;
        }
        let mut new_shares = BTreeMap::new();
        let old_unrestricted = self.unrestricted;
        let mut share = 0_u32;
        for (old_share, station) in &self.shares {
            let scaled = old_share.saturating_mul(30) / runtime;
            share = share.saturating_add(1).max(scaled);
            new_shares.insert(share, *station);
            if *old_share == old_unrestricted {
                self.unrestricted = share;
            }
        }
        self.shares = new_shares;
    }

    fn previous_share(&self, share_end: u32) -> u32 {
        self.shares
            .range((Unbounded, Excluded(share_end)))
            .next_back()
            .map_or(0, |(share, _)| *share)
    }

    fn upper_bound_station(&self, key: u32) -> Option<(u32, u32)> {
        self.shares
            .range((Excluded(key), Unbounded))
            .next()
            .map(|(share, station)| (*share, *station))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FlowStatMap {
    inner: BTreeMap<u32, FlowStat>,
}

impl FlowStatMap {
    #[must_use]
    pub fn iter(&self) -> impl Iterator<Item = (&u32, &FlowStat)> {
        self.inner.iter()
    }

    #[must_use]
    pub fn get(&self, station: &u32) -> Option<&FlowStat> {
        self.inner.get(station)
    }

    #[must_use]
    pub fn get_mut(&mut self, station: &u32) -> Option<&mut FlowStat> {
        self.inner.get_mut(station)
    }

    pub fn add_flow(&mut self, origin: u32, via: u32, flow: u32) {
        if flow == 0 {
            return;
        }
        if let Some(flow_stat) = self.inner.get_mut(&origin) {
            flow_stat.change_share(via, flow as i32);
        } else {
            self.inner.insert(origin, FlowStat::new(via, flow, false));
        }
    }

    pub fn pass_on_flow(&mut self, origin: u32, via: u32, flow: u32) {
        if flow == 0 {
            return;
        }
        if let Some(flow_stat) = self.inner.get_mut(&origin) {
            flow_stat.change_share(via, flow as i32);
            flow_stat.change_share(INVALID_STATION, flow as i32);
        } else {
            let mut flow_stat = FlowStat::new(via, flow, false);
            flow_stat.append_share(INVALID_STATION, flow, false);
            self.inner.insert(origin, flow_stat);
        }
    }

    pub fn finalize_local_consumption(&mut self, self_station: u32) {
        for flow_stat in self.inner.values_mut() {
            let mut local = flow_stat.get_share(INVALID_STATION);
            if local > i32::MAX as u32 {
                flow_stat.change_share(self_station, -i32::MAX);
                flow_stat.change_share(INVALID_STATION, -i32::MAX);
                local -= i32::MAX as u32;
            }
            flow_stat.change_share(self_station, -(local as i32));
            flow_stat.change_share(INVALID_STATION, -(local as i32));
        }
        self.inner.retain(|_, flow_stat| !flow_stat.is_empty());
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::super::rng::Randomizer;
    use super::super::types::INVALID_STATION;
    use super::{FlowStat, FlowStatMap};

    #[test]
    fn linkgraph_parity_get_via_is_deterministic() {
        let mut flow_stat = FlowStat::new(10, 3, false);
        flow_stat.append_share(20, 5, false);
        flow_stat.append_share(30, 7, false);

        let mut rng = Randomizer::new(1);
        let picks: Vec<u32> = (0..4)
            .map(|_| flow_stat.get_via(INVALID_STATION, INVALID_STATION, &mut rng))
            .collect();
        assert_eq!(picks, vec![10, 30, 20, 20]);
    }

    #[test]
    fn linkgraph_parity_finalize_local_consumption_removes_invalid_marker() {
        let mut map = FlowStatMap::default();
        map.add_flow(1, 2, 8);
        map.pass_on_flow(1, 3, 5);
        map.finalize_local_consumption(2);
        let stat = map.get(&1).expect("flow stat exists");
        assert_eq!(stat.get_share(INVALID_STATION), 0);
        assert_eq!(stat.get_share(2), 3);
        assert_eq!(stat.get_share(3), 5);
    }
}
