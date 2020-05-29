use crate::{
  common::{
    lang,
    msg::{ reply }
  },
  collections::base::{ CONFUSION },
  collections::channels::AI_LEARN
};

use serenity::{
  prelude::*,
  model::{channel::{Message}}
};

use serenity::utils::{
  content_safe,
  ContentSafeOptions,
};

use regex::Regex;

use markov::Chain;

/*
use rand::{
  thread_rng,
  seq::SliceRandom
};
*/

pub fn generate(ctx: &Context, msg : &Message, limit: u64) -> String {
  let mut out = String::new();
  if let Some(guild) = msg.guild(&ctx) {
    let mut chain = Chain::new();
    let re = Regex::new(r"<@!?\d{15,20}>").unwrap();
    let msg_content = &msg.content;
    let russian = lang::is_russian(msg_content);
    let guild_id = guild.read().id;
    if let Ok(channels) = guild_id.channels(&ctx) {
      for (chan, _) in channels {
        if let Some(c_name) = chan.name(&ctx) {
          if AI_LEARN.into_iter().any(|&c| c == c_name.as_str()) {
            if let Ok(messages) = chan.messages(&ctx, |r|
              r.limit(limit)
            ) {
              for mmm in messages {
                let mut result = re.replace_all(&mmm.content.as_str(), "").to_string();
                result = result.replace(": ", "");
                result =
                  content_safe(&ctx, &result, &ContentSafeOptions::default()
                    .clean_user(false).clean_channel(true)
                    .clean_everyone(true).clean_here(true));
                if !result.is_empty() && !result.contains("$") {
                  let is_russian = lang::is_russian(result.as_str());
                  if (russian && is_russian)
                  || (!russian && !is_russian) {
                    chain.feed_str(result.as_str());
                  }
                }
              }
            }
          }
        }
      }
      if !russian {
        for confuse in CONFUSION {
          chain.feed_str( confuse );
        }
      }
      chain.feed_str(msg_content.as_str());
      out = chain.generate_str();
    }
  }
  out
}

pub fn response(ctx: &Context, msg : &Message, limit: u64) {
  let answer = generate(&ctx, &msg, limit);
  if !answer.is_empty() {
    reply(&ctx, &msg, answer.as_str());
  }
}