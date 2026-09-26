#!/bin/sh
# Creates the topics from design doc 6.1 (single broker, RF 1). Idempotent.
set -e
BOOTSTRAP="${KAFKA_BOOTSTRAP:-kafka:9092}"
KT=/opt/kafka/bin/kafka-topics.sh

echo "waiting for Kafka at $BOOTSTRAP ..."
until $KT --bootstrap-server "$BOOTSTRAP" --list >/dev/null 2>&1; do sleep 2; done

day=86400000
create() { # name partitions retention_ms
  $KT --bootstrap-server "$BOOTSTRAP" --create --if-not-exists --topic "$1" \
      --partitions "$2" --replication-factor 1 --config retention.ms="$3"
  $KT --bootstrap-server "$BOOTSTRAP" --create --if-not-exists --topic "$1.dlq" \
      --partitions 1 --replication-factor 1 --config retention.ms=$((14 * day))
}

create chat.messages   6 $((7 * day))
create items.events    6 $((7 * day))
create alerts.events   3 $((7 * day))
create access.changes  3 $((30 * day))
create access.audit    3 $((90 * day))
create notify.requests 3 $((3 * day))

$KT --bootstrap-server "$BOOTSTRAP" --list
echo "topics ready"
