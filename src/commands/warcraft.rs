use crate::{
  common::{
    msg::{ channel_message }
  }
};

use serenity::{
  prelude::*,
  model::channel::*,
  framework::standard::{
    Args, CommandResult,
    macros::command
  },
};

use ical;
use reqwest;

use std::io::BufReader;
use tokio::task;

use chrono::prelude::*;
use chrono::{ Duration, Utc };

async fn tour_internal(ctx: &Context, msg: &Message, on : DateTime<Utc>, passed_check : bool) -> CommandResult {
  let reader = task::spawn_blocking(move || {
    let res = reqwest::blocking::get("https://warcraft3.info/ical-events").unwrap();
    let buf = BufReader::new(res);
    ical::IcalParser::new(buf)
  }).await?;

  set! { str_date_now = on.format("%Y%m%d").to_string()
       , str_time_now = on.format("%H%M").to_string() };

  let mut eventos : Vec<(String, String, bool)> = Vec::new();

  for line in reader {
    match line {
      Ok(l) => {
        for e in l.events {
          setm!{ is_today = false
               , tvstr = String::new()
               , evstr = String::new() };

          for ep in e.properties {
            if ep.name == "DTSTART" {
              if let Some(val) = ep.value {
                if val.len() >= 8 {
                  let str_date = &val[..8];

                  let not_passed = if passed_check {
                    if let Ok(local_utc_time) = str_time_now.parse::<i32>() {
                      let str_hour_mins = &val[9..13];
                      if let Ok(event_hours_mins) = str_hour_mins.parse::<i32>() {
                        local_utc_time < event_hours_mins
                      } else {true }
                    } else { true }
                  } else { true };

                  if str_date_now == str_date && not_passed {
                    is_today = true;
                    if val.len() >= 14 {
                      set! { str_hour = &val[9..11]
                           , str_min = &val[11..13] };
                      let msk =
                        if let Ok(str_int) = str_hour.parse::<i32>() {
                          let mut msk_h = str_int + 1;
                          if msk_h >= 24 {
                            msk_h = msk_h - 24;
                          }
                          format!(" ({}:{} MSK)", msk_h.to_string(), str_min)
                        } else { String::from("") };
                      tvstr = format!("• {}:{} CEST {}", str_hour, str_min, msk);
                    }
                  }
                }
              }
            } else {
              if is_today {
                if ep.name == "SUMMARY" {
                  if let Some(val) = &ep.value {
                    evstr = format!("{}", val);
                  }
                }
                if ep.name == "DESCRIPTION" {
                  if let Some(val2) = &ep.value {
                    evstr = format!("{}\n<{}>", evstr, val2);
                  }
                }
              }
            }
          }
          if is_today && !evstr.is_empty() {
            eventos.push((tvstr, evstr, false));
          }
        }
      },
      Err(e) => error!("Failed to parse calendar line {:?}", e)
    }
  }

  if eventos.len() > 0 {
    let date_str_x = on.format("%e-%b (%A)").to_string();
    let title = format!("Events on {}", date_str_x);
    if let Err(why) = msg.channel_id.send_message(&ctx, |m| m
      .embed(|e| e
        .title(title)
        .thumbnail("https://upload.wikimedia.org/wikipedia/en/4/4f/Warcraft_III_Reforged_Logo.png")
        .fields(eventos)
        .colour((255, 192, 203)))).await {
      error!("Error sending help message: {:?}", why);
    }
  } else {
    channel_message(&ctx, &msg,"I am sorry but I can't find anything at the momenet").await;
  }
  Ok(())
}

pub async fn tour(ctx: &Context, msg: &Message, on : DateTime<Utc>) -> CommandResult {
  tour_internal(ctx, msg, on, false).await
}

#[command]
pub async fn yesterday(ctx: &Context, msg: &Message) -> CommandResult {
  let yesterday : DateTime<Utc> = Utc::now() - Duration::days(1); 
  tour(ctx, msg, yesterday).await?;
  if let Err(why) = msg.delete(&ctx).await {
    error!("Error deleting original command {:?}", why);
  }
  Ok(())
}

#[command]
pub async fn today(ctx: &Context, msg: &Message) -> CommandResult {
  let today : DateTime<Utc> = Utc::now(); 
  tour_internal(ctx, msg, today, true).await?;
  if let Err(why) = msg.delete(&ctx).await {
    error!("Error deleting original command {:?}", why);
  }
  Ok(())
}

#[command]
pub async fn tomorrow(ctx: &Context, msg: &Message) -> CommandResult {
  let tomorrow : DateTime<Utc> = Utc::now() + Duration::days(1); 
  tour(ctx, msg, tomorrow).await?;
  if let Err(why) = msg.delete(&ctx).await {
    error!("Error deleting original command {:?}", why);
  }
  Ok(())
}

#[command]
pub async fn weekends(ctx: &Context, msg: &Message) -> CommandResult {
  let mut today : DateTime<Utc> = Utc::now();
  if today.weekday() == Weekday::Sun {
    tour_internal(ctx, msg, today, true).await?;
  } else {
    let is_saturday = today.weekday() == Weekday::Sat;
    if !is_saturday {
      while today.weekday() != Weekday::Sat {
        today = today + Duration::days(1); 
      }
    }
    tour_internal(ctx, msg, today, is_saturday).await?;
    let tomorrow : DateTime<Utc> = today + Duration::days(1); 
    tour(ctx, msg, tomorrow).await?;
  }
  if let Err(why) = msg.delete(&ctx).await {
    error!("Error deleting original command {:?}", why);
  }
  Ok(())
}

#[command]
pub async fn lineup(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
  let mut maps_out : Vec<(String, String, bool)> = Vec::new();
  let text = args.message();

  let check_for_title : Vec<String> =
    text.split("|").map(str::to_string).collect();

  let title = if check_for_title.len() > 1 {
    let s = &check_for_title[0];
    String::from(s.trim())
  } else {
    String::from("Custom lineup")
  };

  let players = if check_for_title.len() > 1 {
    let s = &check_for_title[1];
    String::from(s.trim())
  } else {
    String::from(text)
  };

  let playermap_split = players.split(" ").filter(|x| !x.is_empty());
  let playermap : Vec<String> =
    playermap_split.map(str::to_string).collect();
  for i in (0..(playermap.len() -1)).step_by(2) {
    maps_out.push((playermap[i].clone(), playermap[i + 1].clone(), true));
  }

  let footer = format!("Made by {}", msg.author.name);

  if let Err(why) = msg.channel_id.send_message(&ctx, |m| m
    .embed(|e| e
      .title(title)
      .fields(maps_out)
      .colour((255,182,193))
      .footer(|f| f.text(footer))
    )).await {
    error!("Error sending help message: {:?}", why);
  }
  if let Err(why) = msg.delete(&ctx).await {
    error!("Error deleting original command {:?}", why);
  }
  Ok(())
}
