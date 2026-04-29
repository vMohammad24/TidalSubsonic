pub mod mapping;
pub mod middleware;
pub mod models;
pub mod response;

pub mod browsing;
pub mod interaction;
pub mod media;
pub mod playlists;
pub mod search;
pub mod starred;
pub mod system;
pub mod user;

use actix_web::web;

pub fn config(cfg: &mut web::ServiceConfig) {
	cfg.service(
		web::scope("/rest")
			.wrap(crate::api::subsonic::middleware::SubsonicAuth)
			.route("/ping", web::get().to(system::ping))
			.route("/ping.view", web::get().to(system::ping))
			.route("/getCoverArt", web::get().to(media::get_cover_art))
			.route("/getCoverArt.view", web::get().to(media::get_cover_art))
			.route("/getLicense", web::get().to(system::get_license))
			.route("/getLicense.view", web::get().to(system::get_license))
			.route(
				"/getOpenSubsonicExtensions",
				web::get().to(system::get_open_subsonic_extensions),
			)
			.route(
				"/getOpenSubsonicExtensions.view",
				web::get().to(system::get_open_subsonic_extensions),
			)
			.route("/search3", web::get().to(search::search3))
			.route("/search3.view", web::get().to(search::search3))
			.route("/search", web::get().to(search::search2))
			.route("/search.view", web::get().to(search::search2))
			.route("/search2", web::get().to(search::search2))
			.route("/search2.view", web::get().to(search::search2))
			// Browsing
			.route(
				"/getMusicFolders",
				web::get().to(browsing::get_music_folders),
			)
			.route(
				"/getMusicFolders.view",
				web::get().to(browsing::get_music_folders),
			)
			.route(
				"/getMusicDirectory",
				web::get().to(browsing::get_music_directory),
			)
			.route(
				"/getMusicDirectory.view",
				web::get().to(browsing::get_music_directory),
			)
			.route("/getIndexes", web::get().to(browsing::get_indexes))
			.route("/getIndexes.view", web::get().to(browsing::get_indexes))
			.route("/getTopSongs", web::get().to(browsing::get_top_songs))
			.route("/getTopSongs.view", web::get().to(browsing::get_top_songs))
			.route(
				"/getSimilarSongs",
				web::get().to(browsing::get_similar_songs),
			)
			.route(
				"/getSimilarSongs.view",
				web::get().to(browsing::get_similar_songs),
			)
			.route("/getArtists", web::get().to(browsing::get_artists))
			.route("/getArtists.view", web::get().to(browsing::get_artists))
			.route("/getArtistInfo", web::get().to(browsing::get_artist_info))
			.route(
				"/getArtistInfo.view",
				web::get().to(browsing::get_artist_info),
			)
			.route("/getArtistInfo2", web::get().to(browsing::get_artist_info2))
			.route(
				"/getArtistInfo2.view",
				web::get().to(browsing::get_artist_info2),
			)
			.route("/getRandomSongs", web::get().to(browsing::get_random_songs))
			.route(
				"/getRandomSongs.view",
				web::get().to(browsing::get_random_songs),
			)
			.route(
				"/getSongsByGenre",
				web::get().to(browsing::get_songs_by_genre),
			)
			.route(
				"/getSongsByGenre.view",
				web::get().to(browsing::get_songs_by_genre),
			)
			.route("/getGenres", web::get().to(browsing::get_genres))
			.route("/getGenres.view", web::get().to(browsing::get_genres))
			.route(
				"/getInternetRadioStations",
				web::get().to(browsing::get_internet_radio_stations),
			)
			.route(
				"/getInternetRadioStations.view",
				web::get().to(browsing::get_internet_radio_stations),
			)
			.route("/getAlbumList", web::get().to(browsing::get_album_list))
			.route(
				"/getAlbumList.view",
				web::get().to(browsing::get_album_list),
			)
			.route("/getAlbumList2", web::get().to(browsing::get_album_list))
			.route(
				"/getAlbumList2.view",
				web::get().to(browsing::get_album_list),
			)
			.route("/getAlbum", web::get().to(browsing::get_album))
			.route("/getAlbum.view", web::get().to(browsing::get_album))
			.route("/getArtist", web::get().to(browsing::get_artist))
			.route("/getArtist.view", web::get().to(browsing::get_artist))
			.route("/getAlbumInfo", web::get().to(browsing::get_album_info))
			.route(
				"/getAlbumInfo.view",
				web::get().to(browsing::get_album_info),
			)
			.route("/getAlbumInfo2", web::get().to(browsing::get_album_info2))
			.route(
				"/getAlbumInfo2.view",
				web::get().to(browsing::get_album_info2),
			)
			.route("/getSong", web::get().to(browsing::get_song))
			.route("/getSong.view", web::get().to(browsing::get_song))
			// Playlists
			.route("/getPlaylists", web::get().to(playlists::get_playlists))
			.route(
				"/getPlaylists.view",
				web::get().to(playlists::get_playlists),
			)
			.route("/getPlaylist", web::get().to(playlists::get_playlist))
			.route("/getPlaylist.view", web::get().to(playlists::get_playlist))
			.route("/createPlaylist", web::get().to(playlists::create_playlist))
			.route(
				"/createPlaylist.view",
				web::get().to(playlists::create_playlist),
			)
			.route("/deletePlaylist", web::get().to(playlists::delete_playlist))
			.route(
				"/deletePlaylist.view",
				web::get().to(playlists::delete_playlist),
			)
			.route("/updatePlaylist", web::get().to(playlists::update_playlist))
			.route(
				"/updatePlaylist.view",
				web::get().to(playlists::update_playlist),
			)
			// Media (excluding getCoverArt which is public)
			.route("/stream", web::get().to(media::stream))
			.route("/stream.view", web::get().to(media::stream))
			.route("/download", web::get().to(media::download))
			.route("/download.view", web::get().to(media::download))
			.route("/getLyrics", web::get().to(media::get_lyrics))
			.route("/getLyrics.view", web::get().to(media::get_lyrics))
			.route("/getLyricsBySongId", web::get().to(media::get_lyrics))
			.route("/getLyricsBySongId.view", web::get().to(media::get_lyrics))
			// User
			.route("/getUser", web::get().to(user::get_user))
			.route("/getUser.view", web::get().to(user::get_user))
			.route("/getAvatar", web::get().to(user::get_avatar))
			.route("/getAvatar.view", web::get().to(user::get_avatar))
			// Starred
			.route("/getStarred", web::get().to(starred::get_starred))
			.route("/getStarred.view", web::get().to(starred::get_starred))
			.route("/getStarred2", web::get().to(starred::get_starred2))
			.route("/getStarred2.view", web::get().to(starred::get_starred2))
			.route("/star", web::get().to(starred::star))
			.route("/star.view", web::get().to(starred::star))
			.route("/unstar", web::get().to(starred::unstar))
			.route("/unstar.view", web::get().to(starred::unstar))
			// Interaction
			.route("/setRating", web::get().to(interaction::set_rating))
			.route("/setRating.view", web::get().to(interaction::set_rating))
			.route("/scrobble", web::get().to(interaction::scrobble))
			.route("/scrobble.view", web::get().to(interaction::scrobble))
			.route("/getQueue", web::get().to(interaction::get_queue))
			.route("/getQueue.view", web::get().to(interaction::get_queue))
			.route("/saveQueue", web::get().to(interaction::save_queue))
			.route("/saveQueue.view", web::get().to(interaction::save_queue))
			.route("/getPlayQueue", web::get().to(interaction::get_play_queue))
			.route(
				"/getPlayQueue.view",
				web::get().to(interaction::get_play_queue),
			)
			.route(
				"/savePlayQueue",
				web::get().to(interaction::save_play_queue),
			)
			.route(
				"/savePlayQueue.view",
				web::get().to(interaction::save_play_queue),
			),
	);
}
