import { error, info } from "tauri-plugin-log-api";

export class AudioRecorder {
  private _active = false;

  private _callback: (data: Int16Array) => void;

  private _context: AudioContext | undefined;

  private _stream: MediaStream | undefined;

  private _source: MediaStreamAudioSourceNode | undefined;

  private _recorder: AudioWorkletNode | undefined;

  constructor(callback: (data: Int16Array) => void) {
    this._callback = callback;
  }

  public get active() {
    return this._active;
  }

  public get sampleRate() {
    return this._context?.sampleRate;
  }

  public static get isSupported() {
    return (
      window.isSecureContext &&
      // @ts-ignore-next-line
      (window.AudioContext || window.webkitAudioContext)
    );
  }

  public async start() {
    // Check if we need to recreate - either missing components OR components in bad state
    const needsRecreate =
      !this._context ||
      !this._stream ||
      !this._source ||
      !this._recorder ||
      this._context.state === 'closed' ||
      this._stream.getTracks()[0]?.readyState === 'ended';

    if (needsRecreate) {
      info(`${needsRecreate ? 'Creating' : 'Recreating'} audio context (context state: ${this._context?.state}, track state: ${this._stream?.getTracks()[0]?.readyState})...`);
      try {
        // Clean up old resources if they exist
        if (this._context || this._stream || this._source || this._recorder) {
          this.close();
        }
        await this._createContext();
      } catch (err: any) {
        error(`Error creating context: ${err}`);
        this._active = false;
      }
    } else {
      info(`Reusing existing audio context (state: ${this._context.state})`);
      this._stream.getTracks()[0].enabled = true;
      await this._context.resume();
      info(`Context resumed, new state: ${this._context.state}`);
      this._active = true;
    }
  }

  public async stop() {
    info(`Stopping recorder (active: ${this._active}, context state: ${this._context?.state}, track state: ${this._stream?.getTracks()[0]?.readyState})`);
    this._active = false;
    if (this._stream) {
      this._stream.getTracks()[0].enabled = false;
      info(`Track disabled (new state: ${this._stream.getTracks()[0].readyState})`);
    }
    if (this._context && this._context.state === 'running') {
      await this._context.suspend();
      info(`Context suspended (new state: ${this._context.state})`);
    }
  }

  public close() {
    this._active = false;
    this._stream?.getTracks()[0].stop();
    if (this._recorder) {
      this._recorder.port.onmessage = null;
    }
    this._source?.disconnect();
    this._context?.close();
    this._stream = undefined;
    this._source = undefined;
    this._recorder = undefined;
    this._context = undefined;
  }

  private async _createContext() {
    // @ts-ignore-next-line
    this._context = new (window.AudioContext || window.webkitAudioContext)();
    info("Created audio context");
    this._stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    info("Created stream");

    info(`url: ${import.meta.url}`);
    await this._context.audioWorklet.addModule(
      new URL("./recorder.worklet.js", import.meta.url)
    );
    info("Added worklet");

    this._source = this._context.createMediaStreamSource(this._stream);
    info("Created source");
    this._recorder = new AudioWorkletNode(this._context, "recorder.worklet");
    info("Created recorder");

    this._recorder.port.onmessage = (e) => {
      if (!this._active) {
        return;
      }
      this._callback(e.data);
    };
    this._active = true;
    this._source.connect(this._recorder);
    info("Connected source to recorder");
  }
}
