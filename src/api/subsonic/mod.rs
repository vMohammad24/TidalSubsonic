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

macro_rules! sub_route {
	($cfg:expr, $path:expr, $handler:path) => {
		$cfg = $cfg
			.route($path, actix_web::web::get().to($handler))
			.route(concat!($path, ".view"), actix_web::web::get().to($handler))
	};
}

pub fn config(cfg: &mut web::ServiceConfig) {
	let mut scope = web::scope("/rest").wrap(crate::api::subsonic::middleware::SubsonicAuth);

	sub_route!(scope, "/ping", system::ping);
	sub_route!(scope, "/getCoverArt", media::get_cover_art);
	sub_route!(scope, "/getLicense", system::get_license);
	sub_route!(
		scope,
		"/getOpenSubsonicExtensions",
		system::get_open_subsonic_extensions
	);
	sub_route!(scope, "/search3", search::search3);
	sub_route!(scope, "/search", search::search2);
	sub_route!(scope, "/search2", search::search2);

	// Browsing
	sub_route!(scope, "/getMusicFolders", browsing::get_music_folders);
	sub_route!(scope, "/getMusicDirectory", browsing::get_music_directory);
	sub_route!(scope, "/getIndexes", browsing::get_indexes);
	sub_route!(scope, "/getTopSongs", browsing::get_top_songs);
	sub_route!(scope, "/getSimilarSongs", browsing::get_similar_songs);
	sub_route!(scope, "/getArtists", browsing::get_artists);
	sub_route!(scope, "/getArtistInfo", browsing::get_artist_info);
	sub_route!(scope, "/getArtistInfo2", browsing::get_artist_info);
	sub_route!(scope, "/getRandomSongs", browsing::get_random_songs);
	sub_route!(scope, "/getSongsByGenre", browsing::get_songs_by_genre);
	sub_route!(scope, "/getGenres", browsing::get_genres);
	sub_route!(
		scope,
		"/getInternetRadioStations",
		browsing::get_internet_radio_stations
	);
	sub_route!(scope, "/getAlbumList", browsing::get_album_list);
	sub_route!(scope, "/getAlbumList2", browsing::get_album_list);
	sub_route!(scope, "/getAlbum", browsing::get_album);
	sub_route!(scope, "/getArtist", browsing::get_artist);
	sub_route!(scope, "/getAlbumInfo", browsing::get_album_info);
	sub_route!(scope, "/getAlbumInfo2", browsing::get_album_info);
	sub_route!(scope, "/getSong", browsing::get_song);

	// Playlists
	sub_route!(scope, "/getPlaylists", playlists::get_playlists);
	sub_route!(scope, "/getPlaylist", playlists::get_playlist);
	sub_route!(scope, "/createPlaylist", playlists::create_playlist);
	sub_route!(scope, "/deletePlaylist", playlists::delete_playlist);
	sub_route!(scope, "/updatePlaylist", playlists::update_playlist);

	// Media (excluding getCoverArt which is public)
	sub_route!(scope, "/stream", media::stream);
	sub_route!(scope, "/download", media::download);
	sub_route!(scope, "/getLyrics", media::get_lyrics);
	sub_route!(scope, "/getLyricsBySongId", media::get_lyrics);

	// User
	sub_route!(scope, "/getUser", user::get_user);
	sub_route!(scope, "/getAvatar", user::get_avatar);

	// Starred
	sub_route!(scope, "/getStarred", starred::get_starred);
	sub_route!(scope, "/getStarred2", starred::get_starred);
	sub_route!(scope, "/star", starred::star);
	sub_route!(scope, "/unstar", starred::unstar);

	// Interaction
	sub_route!(scope, "/setRating", interaction::set_rating);
	sub_route!(scope, "/scrobble", interaction::scrobble);
	sub_route!(scope, "/getQueue", interaction::get_queue);
	sub_route!(scope, "/saveQueue", interaction::save_queue);
	sub_route!(scope, "/getPlayQueue", interaction::get_play_queue);
	sub_route!(scope, "/savePlayQueue", interaction::save_play_queue);

	cfg.service(scope);
}
