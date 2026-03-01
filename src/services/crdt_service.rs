/// CRDT service: Hybrid Logical Clock (HLC) and basic CRDT operations
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Hybrid Logical Clock for total ordering
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HybridLogicalClock {
    pub ts: i64,              // Wall-clock timestamp (milliseconds since epoch)
    pub logical_counter: u32, // Logical counter for causality
    pub device_id: String,    // Tie-breaker for deterministic ordering
}

impl HybridLogicalClock {
    /// Create a new HLC with current wall-clock time
    pub fn new(device_id: String) -> Self {
        Self {
            ts: chrono::Utc::now().timestamp_millis(),
            logical_counter: 0,
            device_id,
        }
    }

    /// Advance HLC (increment logical counter or update timestamp)
    /// For batch operations, ensures each operation gets a unique timestamp
    pub fn tick(&mut self) {
        let now = chrono::Utc::now().timestamp_millis();
        if now > self.ts {
            self.ts = now;
            self.logical_counter = 0;
        } else {
            // Increment timestamp by 1ms to ensure uniqueness within batch
            // This simplifies downstream clock comparisons
            self.ts += 1;
            self.logical_counter = 0;
        }
    }
}

impl PartialOrd for HybridLogicalClock {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HybridLogicalClock {
    fn cmp(&self, other: &Self) -> Ordering {
        self.ts
            .cmp(&other.ts)
            .then_with(|| self.logical_counter.cmp(&other.logical_counter))
            .then_with(|| self.device_id.cmp(&other.device_id))
    }
}

/// CRDT operation types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum CrdtOp {
    /// Last-Writer-Wins register (for scalar fields)
    LwwSet {
        field: String,
        value: serde_json::Value,
    },
    /// OR-Set add (for collections)
    OrAdd {
        field: String,
        element: String,
        element_id: String, // unique tag for this add
    },
    /// OR-Set remove (for collections)
    OrRemove { field: String, element_id: String },
    /// PN-Counter increment (for additive counters)
    PnIncrement { field: String, delta: i32 },
    /// Tombstone (soft delete)
    Delete,
}

/// NDJSON operation log entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Operation {
    pub op_id: String,       // ULID of this operation
    pub entity_type: String, // quail | event | egg_record | photo_meta
    pub entity_id: String,   // UUID of the entity
    pub clock: HybridLogicalClock,
    pub op: CrdtOp,
}

impl Operation {
    /// Create a new operation with a fresh HLC tick
    pub fn new(entity_type: String, entity_id: String, device_id: String, op: CrdtOp) -> Self {
        let op_id = ulid::Ulid::new().to_string();
        let mut clock = HybridLogicalClock::new(device_id);
        clock.tick();

        Self {
            op_id,
            entity_type,
            entity_id,
            clock,
            op,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hlc_ordering() {
        let mut clock1 = HybridLogicalClock::new("device1".to_string());
        let mut clock2 = HybridLogicalClock::new("device2".to_string());

        clock1.tick();
        clock2.tick();

        // Assuming clock2 ticks slightly later or same time
        assert!(clock1 <= clock2);
    }
}
