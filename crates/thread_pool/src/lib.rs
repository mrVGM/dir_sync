use std::sync::{mpsc::{channel, Sender}, Arc, Mutex};

use errors::GenericError;

type ReportChannel = Sender<Option<GenericError>>;
type TaskPtr = dyn FnOnce() -> Result<(), GenericError> + Send + 'static;
type TaskChannel = Sender<Option<Box<TaskPtr>>>;

#[derive(Clone)]
pub struct ThreadPool(Arc<ThreadPoolB>);

impl ThreadPool {
    pub fn new(num_threads: u8, report_channel: ReportChannel) -> Self {
        let pool = ThreadPoolB::new(num_threads, report_channel);
        ThreadPool(Arc::new(pool))
    }

    pub fn execute(&self, task: impl FnOnce() -> Result<(), GenericError> + Send + 'static) {
        self.0.execute(task)
    }
}

struct ThreadPoolB {
    num_threads: u8,
    task_channel: TaskChannel,
    report_channel: ReportChannel
}

impl ThreadPoolB {
    fn new(num_threads: u8, report_channel: ReportChannel) -> Self {
        let (task_sender, task_receiver) = channel::<Option<Box<TaskPtr>>>();
        let task_receiver = Arc::new(Mutex::new(task_receiver));
        for _ in 0..num_threads {
            let receiver = Arc::clone(&task_receiver);
            let report_channel = report_channel.clone();

            std::thread::spawn(move || loop {
                let task = {
                    let receiver = &mut *receiver.lock().unwrap();
                    receiver.recv().unwrap()
                };

                if let Some(task) = task {
                    let res = task();
                    match res {
                        Err(err) => {
                            report_channel.send(Some(err)).unwrap();
                        }
                        _ => {}
                    }
                }
                else {
                    break;
                }
            });

        }

        ThreadPoolB {
            num_threads,
            task_channel: task_sender,
            report_channel
        }
    }

    fn execute(&self, task: impl FnOnce() -> Result<(), GenericError> + Send + 'static) {
        let sender = &self.task_channel;
        let b: Box<TaskPtr> = Box::new(task);
        sender.send(Some(b)).unwrap();
    }
}

impl Drop for ThreadPoolB {
    fn drop(&mut self) {
        let num_threads = self.num_threads;
        for _ in 0..num_threads {
            self.task_channel.send(None).unwrap();
        }
    }
}

