import { describe, expect, it } from "vitest";
import { OSC52_MAX_BYTES, parseOsc52 } from "./clipboard";

const b64 = (s: string) => btoa(String.fromCharCode(...new TextEncoder().encode(s)));

describe("OSC 52 parsing", () => {
  it("decodes UTF-8 clipboard writes from any target", () => {
    expect(parseOsc52(`c;${b64("hello from the server")}`)).toBe("hello from the server");
    expect(parseOsc52(`;${b64("ünïcødé 世界")}`)).toBe("ünïcødé 世界");
    expect(parseOsc52(`p;${b64("x")}`)).toBe("x");
  });

  it("never answers clipboard reads and rejects garbage", () => {
    expect(parseOsc52("c;?")).toBeNull();
    expect(parseOsc52("c;")).toBeNull();
    expect(parseOsc52("no-separator")).toBeNull();
    expect(parseOsc52("c;not base64!!")).toBeNull();
    expect(parseOsc52(`c;${btoa("\xff\xfe")}`)).toBeNull(); // invalid UTF-8
  });

  it("caps payload size", () => {
    const big = "A".repeat(Math.ceil((OSC52_MAX_BYTES * 4) / 3) + 100);
    expect(parseOsc52(`c;${big}`)).toBeNull();
  });
});
