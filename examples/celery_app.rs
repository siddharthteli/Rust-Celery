#![allow(non_upper_case_globals)]

use anyhow::Result;
use async_trait::async_trait;
use celery::prelude::*;
use env_logger::Env;
use std::env;
use structopt::StructOpt;
use tokio::time::{self, Duration};

// This generates the task struct and impl with the name set to the function name "add"
#[celery::task]

fn add(x: i32, y: i32) -> TaskResult<i32> {
    Ok(x + y)
}

// Demonstrates a task that raises an error, and also how to customize task options.
// In this case we override the default `max_retries`.
#[celery::task(max_retries = 3)]
async fn buggy_task() -> TaskResult<()> {
    let data = tokio::fs::read("this-file-doesn't-exist")
        .await
        .with_unexpected_err(|| {
            "This error is part of the example, it is used to showcase error handling"
        })?;
    println!("Read {} bytes", data.len());
    Ok(())
}

// Demonstrates a long running IO-bound task. By increasing the prefetch count, an arbitrary
// number of these number can execute concurrently.
#[celery::task(max_retries = 2)]
async fn long_running_task(secs: Option<u64>) {
    let secs = secs.unwrap_or(10);
    time::sleep(Duration::from_secs(secs)).await;
}

// Demonstrates a task that is bound to the task instance, i.e. runs as an instance method.
#[celery::task(bind = true)]
fn bound_task(task: &Self) {
    // Print some info about the request for debugging.
    println!("{:?}", task.request.origin);
    println!("{:?}", task.request.hostname);
}

#[derive(Debug, StructOpt)]
#[structopt(
    name = "celery_app",
    about = "Run a Rust Celery producer or consumer.",
    setting = structopt::clap::AppSettings::ColoredHelp,
)]
enum CeleryOpt {
    Producer,
    Consumer,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    println!("1st");
    let opt: Vec<String> = env::args().collect();
    let celeryopt: CeleryOpt = match opt[1].as_str() {
        "Producer" => CeleryOpt::Producer,
        "Consumer" => CeleryOpt::Consumer,
        _ => CeleryOpt::Consumer,
    };

    println!("-----Value of selection:{:?}-----", celeryopt);

    let my_app = celery::app!(
        broker = AMQPBroker { std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/my_vhost".into()) },
        tasks = [add],
        task_routes = ["*" => "celery"],
    )
    .await?;

    match celeryopt {
        CeleryOpt::Consumer => {
            println!("-----Consumer selected-----");
            my_app.display_pretty().await;
            my_app.consume_from(&["celery"]).await?;
        }
        CeleryOpt::Producer => {
            println!("-----Producer selected-----");
            my_app.send_task(add::new(1, 2)).await?;
        }
    }
    my_app.close().await?;
    Ok(())
}
