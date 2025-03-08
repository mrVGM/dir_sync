use std::sync::{mpsc::{channel, Receiver, Sender}, Arc, LazyLock, Mutex};

use errors::GenericError;

type ReportSender = Sender<Option<GenericError>>;
type ReportReceiver = Arc<Mutex<Receiver<Option<GenericError>>>>;

type TaskPtr = dyn FnOnce() -> Result<(), GenericError> + Send + 'static;
type TaskChannel = Sender<Option<Box<TaskPtr>>>;

static REPORT_CHANNEL: LazyLock<(ReportSender, ReportReceiver)> = LazyLock::new(|| {
    let (sender, receiver) = channel();
    let receiver = Arc::new(Mutex::new(receiver));

    (sender, receiver)
});

pub fn get_report_receiver() -> ReportReceiver {
    Arc::clone(&REPORT_CHANNEL.1)
}

pub fn get_report_channel() -> ReportSender {
    REPORT_CHANNEL.0.clone()
}

#[derive(Clone)]
pub struct ThreadPool(Arc<ThreadPoolInternal>);

impl ThreadPool {
    pub fn new(num_threads: u8) -> Self {
        let pool = ThreadPoolInternal::new(num_threads);
        ThreadPool(Arc::new(pool))
    }

    pub fn execute(&self, task: impl FnOnce() -> Result<(), GenericError> + Send + 'static) {
        self.0.execute(task)
    }
}

struct ThreadPoolInternal {
    num_threads: u8,
    task_channel: TaskChannel,
}

impl ThreadPoolInternal {
    fn new(num_threads: u8) -> Self {
        let (task_sender, task_receiver) = channel::<Option<Box<TaskPtr>>>();
        let task_receiver = Arc::new(Mutex::new(task_receiver));
        for _ in 0..num_threads {
            let receiver = Arc::clone(&task_receiver);
            let report_channel = get_report_channel();

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

        ThreadPoolInternal {
            num_threads,
            task_channel: task_sender,
        }
    }

    fn execute(&self, task: impl FnOnce() -> Result<(), GenericError> + Send + 'static) {
        let sender = &self.task_channel;
        let b: Box<TaskPtr> = Box::new(task);
        sender.send(Some(b)).unwrap();
    }
}

impl Drop for ThreadPoolInternal {
    fn drop(&mut self) {
        let num_threads = self.num_threads;
        for _ in 0..num_threads {
            self.task_channel.send(None).unwrap();
        }
    }
}

