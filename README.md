# deluge-maintain

A service that puts deluge on autopilot using retention policies.

## Usage

Container images are published automatically. A `deluge-maintain.toml` is required.
See [deluge-maintain.toml](deluge-maintain.toml) for an example.

```bash
docker run \
  -v /path/to/deluge-maintain.toml:/config/deluge-maintain.toml \
  ghcr.io/silvenga/deluge-maintain:latest
```

## Configuration

One or more retention policies drive are executed against each host in order. Checking if any torrents needed to be
deleted to match the configured policy.

Each policy has four parts:

- name: A human-readable name, shown in logging.
- cron: The schedule to apply the policy.
- conditions: When to execute the policy (if any of the conditions are met).
- filter: When executing the policy, which torrents to consider for deletion.

| Condition         | Description                                                                                        |
|-------------------|----------------------------------------------------------------------------------------------------|
| `available_space` | The available disk space on the disk where the default download folder exists.                     |
| `used_space`      | The total disk space "wanted" by the registered torrents (closest analog to "space used on disk"). |
| `total_count`     | The maximum number of torrents.                                                                    |

| Filter             | Description                                                                                                                      |
|--------------------|----------------------------------------------------------------------------------------------------------------------------------|
| `age`              | The minimum age of the torrent, from the date it was added to Deluge.                                                            |
| `ratio`            | The minimum ratio of the torrent.                                                                                                |
| `completed`        | If the torrent is completely downloaded, defaults to true.                                                                       |
| `min_total_seeds`  | The minimum detected seeders of the torrent.                                                                                     |
| `min_availability` | The minimum availablity (0.0-1.0), defaults to 1.0, meaning only torrents that have a complete copy in the swarm are considered. |

When applying retention policies, `deluge-maintain` attempts to maximize swarm health, favoring the removal of
well-seeded and healthy torrents, over torrents with smaller swarms. 
