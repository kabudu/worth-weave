import "@testing-library/jest-dom/vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";

import { UpdateBanner } from "./UpdateBanner";

vi.mock("@tauri-apps/api/core", () => ({ isTauri: () => true }));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: vi.fn() }));
vi.mock("@tauri-apps/plugin-updater", () => ({ check: vi.fn() }));

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

test("offers a verified update and restarts after installation", async () => {
  const update = {
    version: "0.2.0",
    close: vi.fn(),
    downloadAndInstall: vi.fn(async (onEvent?: (event: DownloadEvent) => void) => {
      onEvent?.({ event: "Started", data: { contentLength: 100 } });
      onEvent?.({ event: "Progress", data: { chunkLength: 100 } });
      onEvent?.({ event: "Finished" });
    }),
  } as unknown as Update;
  vi.mocked(check).mockResolvedValue(update);

  render(<UpdateBanner />);

  expect(await screen.findByText("Worthweave 0.2.0 is ready")).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "Update and restart" }));
  await waitFor(() => expect(update.downloadAndInstall).toHaveBeenCalledOnce());
  await waitFor(() => expect(relaunch).toHaveBeenCalledOnce());
});
