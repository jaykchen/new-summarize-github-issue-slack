use dotenv::dotenv;
use github_flows::{get_octo, GithubLogin::Provided};
use slack_flows::{listen_to_channel, send_message_to_channel};
use std::env;
// use tiktoken_rs::cl100k_base;
use http_req::{request::Method, request::Request, uri::Uri};
use serde::{Deserialize, Serialize};
use serde_json::Value;


#[no_mangle]
pub fn run() {
    dotenv().ok();

    let trigger_word = env::var("trigger_word").unwrap_or("flows summarize".to_string());
    let slack_workspace = env::var("slack_workspace").unwrap_or("secondstate".to_string());
    let slack_channel = env::var("slack_channel").unwrap_or("github-status".to_string());

    listen_to_channel(&slack_workspace, &slack_channel, |sm| {
        handler(&trigger_word, &slack_workspace, &slack_channel, &sm)
    });
}

#[tokio::main(flavor = "current_thread")]
async fn handler(trigger: &str, workspace: &str, channel: &str, sm: &slack_flows::SlackMessage) {
    let openai_key_name = env::var("openai_key_name").unwrap_or("secondstate".to_string());

    let github_login = env::var("github_login").unwrap_or("alabulei1".to_string());
    let github_owner = env::var("github_owner").unwrap_or("alabulei1".to_string());
    let github_repo = env::var("github_repo").unwrap_or("a-test".to_string());

    let number = env::var("number").unwrap().parse::<u64>().unwrap_or(2445);
    send_message_to_channel("ik8", "ch_in", sm.text.clone());

    if sm.text.to_lowercase().contains(trigger) {
        let octocrab = get_octo(&Provided(github_login));
        let issues_handle = octocrab.issues(&github_owner, &github_repo);

        let issue = issues_handle.get(number).await.unwrap();
        let issue_creator_name = issue.user.login;
        let mut issue_creator_role = "".to_string();
        issue_creator_role = issue.author_association;
        let issue_title = issue.title;
        let issue_number = issue.number;
        let issue_body = issue.body.unwrap_or("".to_string());
        let issue_url = issue.html_url;
        let labels = issue
            .labels
            .into_iter()
            .map(|lab| lab.name)
            .collect::<Vec<String>>()
            .join(", ");

        let head = issue_body.chars().take(50).collect::<String>();
        send_message_to_channel("ik8", "ch_in", head);

        // let bpe = cl100k_base().unwrap();

        let mut feed_tokens_map = String::new();
        // let mut feed_tokens_map = Vec::new();

        let issue_creator_input = format!("issue creator {issue_creator_name} has role {issue_creator_role}, filed the issue titled {issue_title}, with labels {labels}, posting: {issue_body}");

        // let mut tokens = bpe.encode_ordinary(&issue_creator_input);
        // feed_tokens_map.append(&mut tokens);
        feed_tokens_map.push_str(&issue_creator_input);

        match issues_handle.list_comments(number).send().await {
            Ok(pages) => {
                for comment in pages.items {
                    let comment_body = comment.body.unwrap();
                    let commenter = comment.user.login;
                    let commenter_input = format!("{commenter} commented: {comment_body}");
                    // let mut tokens = bpe.encode_ordinary(&commenter_input);
                    // feed_tokens_map.append(&mut tokens);
                    feed_tokens_map.push_str(&commenter_input);
                }
            }

            Err(_e) => {}
        }

        // let mut openai = OpenAIFlows::new();
        // openai.set_retry_times(2);
        let system = &format!("You are the co-owner of a github repo, you monitor new issues by analyzing the title, body text, labels and its context");

        // let co = ChatOptions {
        //     model: ChatModel::GPT35Turbo,
        //     restart: true,
        //     system_prompt: Some(system),
        // };
        // let chat_id = format!("ISSUE#{issue_number}");

        let total_tokens_count = feed_tokens_map.len();
        let mut _summary = "".to_string();

        if total_tokens_count > 1800 {
            let mut token_vec = feed_tokens_map.split_whitespace().collect::<Vec<&str>>();
            let mut map_out = "".to_string();

            while !token_vec.is_empty() {
                let drain_to = std::cmp::min(token_vec.len(), 3000);
                let token_chunk = token_vec.drain(0..drain_to).collect::<Vec<_>>();

                let text_chunk = token_chunk.join(" ");
                // let text_chunk = bpe.decode(token_chunk).unwrap();

                let map_question = format!("The issue is titled {issue_title}, with one chunk of the body text or comment text {text_chunk}. Please focus on the main points of the comment, any proposed solutions, and any consensus or disagreements among the commenters. Please summarize key information in this section.");

             let r = custom_gpt(&system, &map_question, 128).await;
              send_message_to_channel("ik8", "ch_out", r.clone());

                map_out.push_str(&r);

                // match openai.chat_completion(&chat_id, &map_question, &co).await {
                //     Ok(r) => {
                //         send_message_to_channel("ik8", "ch_out", r.choice.clone());

                //         map_out.push_str(r.choice.trim());
                //     }
                //     Err(_e) => {}
                // }
            }

            let reduce_question = format!("{issue_creator_name} with role {issue_creator_role} filed the issue titled {issue_title}, labeled {labels}, here are the key info you extracted from issue body text and comments in chunks {map_out}, please focus on the main points of the comments, any proposed solutions, and any consensus or disagreements among the commenters. Please make a concise summary for this issue to facilitate the next action.");

            _summary = custom_gpt(&system, &reduce_question, 256).await.to_string();;
            send_message_to_channel("ik8", "ch_out", _summary.clone());

            // match openai
            //     .chat_completion(&chat_id, &reduce_question, &co)
            //     .await
            // {
            //     Ok(r) => {
            //         _summary = r.choice.trim();
            //         return;
            //     }
            //     Err(_e) => {}
            // }
        } else {
            let issue_body = feed_tokens_map;
            // let issue_body = bpe.decode(feed_tokens_map).unwrap();

            let question = format!("{issue_body}, please focus on the main points of the comments, any proposed solutions, and any consensus or disagreements among the commenters. Please make a concise summary for this issue to facilitate the next action.");

            _summary = custom_gpt(&system, &question, 256).await.to_string();;
            send_message_to_channel("ik8", "ch_out", _summary.clone());


            // match openai.chat_completion(&chat_id, &question, &co).await {
            //     Ok(r) => {
            //         _summary = r.choice.trim();
            //         send_message_to_channel("ik8", "ch_out", r.choice.clone());

            //         return;
            //     }
            //     Err(_e) => {}
            // }
        }

        let text = format!("Issue Summary:\n{}\n{}", _summary, issue_url);
        send_message_to_channel(&workspace, &channel, text);
    }
}


pub async fn custom_gpt(sys_prompt: &str, u_prompt: &str, m_token: u16) -> String {
    let system_prompt = serde_json::json!(
        {"role": "system", "content": sys_prompt}
    );
    let user_prompt = serde_json::json!(
        {"role": "user", "content": u_prompt}
    );

    if let Ok((res, _)) = chat(vec![system_prompt, user_prompt], m_token).await {
        return res;
    } else {
        return "".to_string();
    }
}

pub async fn chat(message_obj: Vec<Value>, m_token: u16) -> Result<(String, String), anyhow::Error> {
    dotenv().ok();
    let api_token = env::var("OPENAI_API_TOKEN")?;

    let params = serde_json::json!({
      "model": "gpt-3.5-turbo",
      "messages": message_obj,
      "temperature": 0.7,
      "top_p": 1,
      "n": 1,
      "stream": false,
      "max_tokens": m_token,
      "presence_penalty": 0,
      "frequency_penalty": 0,
      "stop": "\n"
    });

    let uri = "https://api.openai.com/v1/chat/completions";

    let uri = Uri::try_from(uri)?;
    let mut writer = Vec::new();
    let body = serde_json::to_vec(&params)?;

    let bearer_token = format!("Bearer {}", api_token);
    let _response = Request::new(&uri)
        .method(Method::POST)
        .header("Authorization", &bearer_token)
        .header("Content-Type", "application/json")
        .header("Content-Length", &body.len())
        .body(&body)
        .send(&mut writer)?;

    let res = serde_json::from_slice::<ChatResponse>(&writer)?;
    let finish_reason = res.choices[0].finish_reason.clone();
    Ok((res.choices[0].message.content.to_string(), finish_reason))
}

#[derive(Deserialize)]
pub struct ChatResponse {
    pub id: String,
    pub choices: Vec<Choice>,
}

#[derive(Deserialize)]
pub struct Choice {
    pub index: u32,
    pub message: Message,
    pub finish_reason: String,
}

#[derive(Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}
