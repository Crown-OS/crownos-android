package org.crownos.agent.task;

/** How much the agent controls task geometry. */
public enum TaskPlacement {
    /** The agent is the task organizer: it sizes tasks into slots and places their leashes. */
    ORGANIZED,
    /** Safe mode, or no organizer on this image: tasks are reported and launched, never moved. */
    OBSERVED
}
