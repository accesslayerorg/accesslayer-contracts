# Event Stability Guarantees

This document clarifies which aspects of emitted events are considered stable across contract upgrades and what changes are considered breaking for event consumers (indexers, monitoring tools, etc.).

## Stable Guarantees

The following aspects of events are guaranteed to remain stable across contract upgrades:

### Topic Identifiers

- **Topic 0 (Event Name)**: The `Symbol` identifier for an event type (e.g., `buy`, `register`) is stable. Once an event name is introduced, its identifier will not change.
- **Topic Index Meaning**: The semantic meaning of a topic at a given index is stable. For example, if Topic 1 is the `creator` address for the `buy` event, it will always be the `creator` address at that index.
- **Topic Order**: The order of topics is stable. New topics may only be appended at the end; existing topics will not be reordered.

### Data Field Order

- **Field Order**: The order of fields in event data payloads (both structs and tuples) is stable. Once a field is introduced at a position, it will remain at that position.
- **Appends Only**: New fields may only be added to the end of a struct or tuple. Existing fields will not be removed or reordered.
- **Field Types**: The type of a field at a given position is stable. A field that is `Address` at position N will always be `Address` at position N.

## Breaking Changes

The following changes to events are considered breaking for event consumers:

### Topic Changes

- **Renaming Events**: Changing the `Symbol` identifier for an event (e.g., renaming `buy` to `purchase`) is breaking.
- **Reordering Topics**: Changing the order of topics (e.g., moving `buyer` from Topic 2 to Topic 1) is breaking.
- **Removing Topics**: Removing a topic from an event is breaking.
- **Changing Topic Types**: Changing the type of a topic at a given index (e.g., changing Topic 1 from `Address` to `u32`) is breaking.

### Data Payload Changes

- **Reordering Fields**: Changing the order of fields in event data is breaking.
- **Removing Fields**: Removing a field from event data is breaking.
- **Changing Field Types**: Changing the type of a field at a given position is breaking.
- **Changing Data Structure**: Converting a struct payload to a tuple (or vice versa) for an existing event is breaking.

## Non-Breaking Changes

The following changes to events are considered non-breaking for event consumers:

### Topic Changes

- **Appending Topics**: Adding a new topic at the end of the topic list is non-breaking. Consumers that only read existing topics will continue to work.

### Data Payload Changes

- **Appending Fields**: Adding a new field at the end of a struct or tuple is non-breaking. Consumers that only read existing fields will continue to work.
- **Adding Optional Fields**: Adding optional fields (e.g., `Option<T>`) at the end of a struct is non-breaking.

## Guidance for Event Consumers

### Robust Parsing

Event consumers should implement robust parsing strategies:

1. **Field Access by Position**: Access fields by their index/position rather than by name when parsing tuples.
2. **Graceful Degradation**: When new fields are appended, consumers should ignore unknown fields rather than failing.
3. **Version Awareness**: Consumers should track the protocol state version via `get_protocol_state_version` to detect when event schema changes may have occurred.

### Monitoring for Changes

Indexers and monitoring tools should:

1. **Log Unknown Fields**: When encountering unknown fields in event data, log a warning but continue processing.
2. **Track Protocol Version**: Use `get_protocol_state_version` to detect when contract upgrades may have introduced event changes.
3. **Validate Topic Structure**: Verify that topics match expected types and positions before processing.

## Versioning Strategy

When event schemas must change in a breaking way:

1. **Increment Protocol State Version**: The `PROTOCOL_STATE_VERSION` constant should be incremented to signal to consumers that event parsing logic may need updates.
2. **Deprecation Period**: Where possible, maintain backward compatibility by emitting both old and new event formats during a transition period.
3. **Documentation**: Update this document and the event naming conventions document to reflect the new schema.
