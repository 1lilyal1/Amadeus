use crate::{
  types::{
    options::IOptions,
    tracking::TrackingGame,
    twitch::Twitch,
    goodgame::GoodGameData
  },
  common::help::channel::channel_by_name,
  stains::cyber
};

use serenity::{
  prelude::*,
  model::{
    id::ChannelId,
    channel::GuildChannel
  }
};

use std::{
  collections::HashMap,
  time
};

pub async fn activate_games_tracking(
                     ctx:       &Context
                   , channels:  &HashMap<ChannelId, GuildChannel>
                   , options:   &IOptions ) {
  if let Some((channel, _)) = channel_by_name(&ctx, &channels, "log").await {

    // Delete live games from log channel (if some)
    if let Ok(vec_msg) = channel.messages(&ctx, |g| g.limit(50)).await {
      let mut vec_id = Vec::new();
      for message in vec_msg {
        for embed in message.embeds {
          if let Some(title) = embed.title {
            if title == "LIVE" || title == "JUST STARTED" {
              vec_id.push(message.id);
              break;
            }
          }
        }
      }
      if !vec_id.is_empty() {
        match channel.delete_messages(&ctx, vec_id.as_slice()).await {
          Ok(nothing)  => nothing,
          Err(err) => warn!("Failed to clean live messages {}", err),
        };
      }
    }

    set!{ ch_deref      = *channel
        , ctx_clone     = ctx.clone()
        , options_clone = options.clone() };

    tokio::spawn(async move {
      let mut games_lock = cyber::team_checker::GAMES.lock().await;
      loop {
        let mut k_to_del : Vec<String> = Vec::new();
        for (k, track) in games_lock.iter_mut() {
          if track.passed_time < 666 {
            track.passed_time += 1;
            track.still_live = false;
          } else {
            k_to_del.push(k.clone());
          }
        }
        for ktd in k_to_del {
          warn!("match {} out with timeout", ktd);
          games_lock.remove(ktd.as_str());
        }
        info!("check");

        let our_gsx = cyber::team_checker::check( &ctx_clone
                                                , *ch_deref.as_u64()
                                                , options_clone.guild
                                                , &mut games_lock
                                                ).await;
        for game in our_gsx {
          let game_key = game.key.clone();
          if let Ok(user) = ctx_clone.http.get_user(game.player.discord).await {

            setm!{ twitch_live        = false
                  , additional_fields  = Vec::new()
                  , image              = None
                  , em_url             = None };

            if game.player.streams.is_some() {
              let streams = game.player.streams.clone().unwrap();
              if streams.twitch.is_some() {
                let client = reqwest::Client::new();
                let getq = format!("https://api.twitch.tv/helix/streams?user_login={}", streams.twitch.unwrap().as_str());
                if let Ok(res) = client
                  .get(getq.as_str())
                  .header("Authorization", options_clone.twitch_oauth.clone())
                  .header("Client-ID", options_clone.twitch_client_id.clone())
                  .send().await {
                  match res.json::<Twitch>().await {
                    Ok(t) => {
                      if !t.data.is_empty() {
                        let twd = &t.data[0];
                        let url = format!("https://www.twitch.tv/{}", twd.user_name);
                        let pic = twd.thumbnail_url.replace("{width}", "800")
                                                    .replace("{height}", "450");
                        if twd.type_string == "live" {
                          additional_fields.push(("Live on twitch", twd.title.clone(), false));
                          image       = Some(pic);
                          em_url      = Some(url);
                          twitch_live = true;
                        }
                      }
                    }, Err(why) => {
                      error!("Failed to parse twitch structs {:?}", why);
                    }
                  }
                }
              }
              if streams.ggru.is_some() {
                let ggru = streams.ggru.clone().unwrap();
                let ggru_link = format!("http://api2.goodgame.ru/v2/streams/{}", ggru.as_str());
                if let Ok(gg) = reqwest::get(ggru_link.as_str()).await {
                  match gg.json::<GoodGameData>().await {
                    Ok(ggdata) => {
                      if ggdata.status == "Live" {
                        let url = format!("https://goodgame.ru/channel/{}", ggru.as_str());
                        if twitch_live {
                          let titurl =
                            format!("{}\n{}", ggdata.channel.title.as_str(), url);
                          additional_fields.push(("Live on ggru", titurl, false));
                        } else {
                          additional_fields.push(("Live on ggru", ggdata.channel.title.clone(), false));
                          image  = Some(ggdata.channel.thumb.clone());
                          em_url = Some(url);
                        }
                      }
                    }, Err(why) => {
                      error!("Failed to parse good game structs {:?}", why);
                    }
                  };
                }
              }
            }

            match ch_deref.send_message(&ctx_clone, |m| m
              .embed(|e| {
                let mut e = e
                  .title("JUST STARTED")
                  .author(|a| a.icon_url(&user.face()).name(&user.name))
                  .description(game.description.as_str());
                if !additional_fields.is_empty() {
                  e = e.fields(additional_fields);
                }
                if let Some(some_image) = image {
                  e = e.image(some_image);
                }
                if let Some(some_url) = em_url {
                  e = e.url(some_url);
                }
                e
              }
            )).await {
              Ok(msg_id) => {
                games_lock.insert(game_key, TrackingGame {
                  tracking_msg_id: *msg_id.id.as_u64(),
                  passed_time: 0,
                  still_live: false,
                  player: game.player }
                );
              },
              Err(why) => {
                error!("Failed to post live match {:?}", why);
              }
            }
          }
        }
        tokio::time::delay_for(time::Duration::from_secs(30)).await;
      }
    });
  }
}
