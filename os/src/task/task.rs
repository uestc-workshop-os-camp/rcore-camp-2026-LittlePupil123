//! Types related to task management

use super::TaskContext;

/// The task control block (TCB) of a task.
#[derive(Copy, Clone)]
pub struct TaskControlBlock { //任务控制块，包含任务状态和任务上下文
    /// The task status in it's lifecycle
    pub task_status: TaskStatus,
    /// The task context
    pub task_cx: TaskContext,
}

/// The status of a task
#[derive(Copy, Clone, PartialEq)]
pub enum TaskStatus {
    /// uninitialized
    UnInit, //未初始化
    /// ready to run
    Ready, //准备运行
    /// running
    Running, //正在运行
    /// exited
    Exited, //初始化
}
