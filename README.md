# TidalSubSonic

This is an open-source verison of an old project of mine, based on the Subsonic music server API structure, but only uses Tidal as it's source of music.

## Features

### Core
*   **Amazing Client Support:** works with almost any client (create an issue if not supported) (e.g., Symfonium, Novic, Feishin).
*   **Complete Library:** Browse your Tidal library natively as if it were a local Subsonic server. Supports fetching artists, albums, top songs, and similar tracks.
*   **Playlists:** almost full (you cant add tracks currently) support for playlists, automatically synchronized with your Tidal account.
*   **Favorites:** star or unstar any album/track/artist from your Subsonic client and have them instantly reflect in your Tidal favorites.

### Playback
*   **Lossless & HI-Res Streaming:** extracts high-quality audio streams (flac/m4a) directly from Tidal, supporting dolby atmos tracks.
*   **Media Downloading:** download any number of tracks without consuming any storage (on the fly zip for albums, direct stream for tracks).
*   **Lyrics:** fetches and serves synchronized or static lyrics directly from Tidal to your Subsonic client.
*   **Caching:** Built with `moka` to aggressively cache API responses, album metadata, and track information, drastically reducing Tidal API limits and ensuring BLAZINGLY-fast client load times.

### Integrations
*   **Last.fm:** link your last.fm account via oauth to generate dynamic, personalized Subsonic feeds (Random, Recent, and Frequent albums) based on your scrobble history.

### User Management
*   **Multi-User Sharing:** create multiple Subsonic user profiles that all piggyback off a single linked Tidal master account (having the ability to share your account with friends).
*   **Data Export:** supports exporting all of your users' data for backup or migration to a new instance.
*   **Authentication:** secure user authentication with full encryption of tidal tokens (access/refresh) and subsonic user passwords.

## TODO
- [ ] **Playlists:** implement adding/removing tracks to a playlist.
- [ ] **API Migration:** migrate from old Tidal APIs to the OpenAPI endpoints (mostly just search).
- [ ] **Local Playlists/Favorites:** allow toggling favorites & playlists from the local database instead of syncing directly from Tidal (like old tss).
- [ ] **Radio / Mixes:** implement `getInternetRadioStations` to return user mixes (or last.fm recommendations?).
- [ ] **OpenSubsonic Extensions:** implement extensions (`apiKeyAuth`, `songLyrics`, `indexBasedQueue`, `formPost`).
- [ ] **Testing:** add unit/integration tests.


## LICENSE
This project is licensed under the GNU Affero General Public License v3.0 (AGPL-3.0). See the `[LICENSE](LICENSE)` file for details.

## Contributing

Contributions, issues and feature requests are highly welcome and appreciated.

1. Fork the project.

2. Create a branch. You can do this with `git checkout -b feature/myFeature`.

3. Commit your changes. You can do this with `git commit -m 'fix trust'`.
