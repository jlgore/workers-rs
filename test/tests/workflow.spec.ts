import { describe, expect, test } from "vitest";
import * as uuid from "uuid";
import { mf } from "./mf";

interface WorkflowStatus<T> {
  status: string;
  output: T | null;
  error?: { name: string; message: string };
}

interface WorkflowInstance<T> {
  id: string;
  status(): Promise<WorkflowStatus<T>>;
}

interface WorkflowBinding<I, O> {
  create(options: { id: string; params: I }): Promise<WorkflowInstance<O>>;
}

async function waitForTerminal<T>(instance: WorkflowInstance<T>) {
  let status = await instance.status();
  for (let attempts = 0; attempts < 100 && !["complete", "errored", "terminated"].includes(status.status); attempts++) {
    await new Promise((resolve) => setTimeout(resolve, 50));
    status = await instance.status();
  }
  return status;
}

describe("workflow entrypoint", () => {
  test("runs a typed Rust step and persists its output", async () => {
    const bindings = await mf.getBindings<{
      TEST_WORKFLOW: WorkflowBinding<{ value: string }, { value: string }>;
    }>();
    const instance = await bindings.TEST_WORKFLOW.create({
      id: uuid.v4(),
      params: { value: "from Rust" },
    });

    const status = await waitForTerminal(instance);

    expect(status.error).toBeUndefined();
    expect(status).toMatchObject({
      status: "complete",
      output: { value: "from Rust" },
    });
  });

  test("preserves NonRetryableError identity across the Rust bridge", async () => {
    const bindings = await mf.getBindings<{
      TEST_WORKFLOW: WorkflowBinding<{ value: string }, { value: string }>;
    }>();
    const instance = await bindings.TEST_WORKFLOW.create({
      id: uuid.v4(),
      params: { value: "non-retryable" },
    });

    const status = await waitForTerminal(instance);

    expect(status.status).toBe("errored");
    expect(status.error?.message).toContain("NonRetryableError");
  });
});
