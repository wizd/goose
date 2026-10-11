import { useCallback, useEffect, useRef, useState } from 'react';
import { Check, Loader2 } from 'lucide-react';
import type { IntlShape } from 'react-intl';
import CreatableSelect from 'react-select/creatable';
import {
  acpClearDefaults,
  acpListProviderDetails,
  acpListSupportedModels,
  acpReadDefaults,
  acpVerifyService,
  type ServiceVerifyAudio,
} from '../../../acp/providers';
import { useConfig } from '../../ConfigContext';
import { useModelAndProvider } from '../../ModelAndProviderContext';
import { Button } from '../../ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '../../ui/card';
import { Select } from '../../ui/Select';
import { defineMessages, useIntl } from '../../../i18n';

const i18n = defineMessages({
  title: { id: 'serviceSettings.title', defaultMessage: 'Services' },
  description: {
    id: 'serviceSettings.description',
    defaultMessage:
      'Choose a provider and model for each service. The agent uses these when it needs that capability.',
  },
  providerPlaceholder: {
    id: 'serviceSettings.providerPlaceholder',
    defaultMessage: 'Provider',
  },
  modelPlaceholder: { id: 'serviceSettings.modelPlaceholder', defaultMessage: 'Model' },
  notWired: { id: 'serviceSettings.notWired', defaultMessage: 'Not connected yet' },
  chatName: { id: 'serviceSettings.chatName', defaultMessage: 'Chat' },
  chatDescription: {
    id: 'serviceSettings.chatDescription',
    defaultMessage: 'The model used for conversation. This is the same choice as Switch models.',
  },
  visionName: { id: 'serviceSettings.visionName', defaultMessage: 'Vision' },
  visionDescription: {
    id: 'serviceSettings.visionDescription',
    defaultMessage: 'Describes images through describe_image when the chat model cannot see them.',
  },
  sttName: { id: 'serviceSettings.sttName', defaultMessage: 'Speech to text' },
  sttDescription: {
    id: 'serviceSettings.sttDescription',
    defaultMessage: 'Transcription used when dictation is set to the service configuration.',
  },
  realtimeName: { id: 'serviceSettings.realtimeName', defaultMessage: 'Realtime voice' },
  realtimeDescription: {
    id: 'serviceSettings.realtimeDescription',
    defaultMessage: 'Saved for a later realtime voice provider. Goose does not call it yet.',
  },
  ttsName: { id: 'serviceSettings.ttsName', defaultMessage: 'Text to speech' },
  ttsDescription: {
    id: 'serviceSettings.ttsDescription',
    defaultMessage: 'Speaks text through the text_to_speech tool.',
  },
  embeddingName: { id: 'serviceSettings.embeddingName', defaultMessage: 'Embeddings' },
  embeddingDescription: {
    id: 'serviceSettings.embeddingDescription',
    defaultMessage: 'Turns text into vectors through the embed_text tool.',
  },
  imageName: { id: 'serviceSettings.imageName', defaultMessage: 'Image generation' },
  imageDescription: {
    id: 'serviceSettings.imageDescription',
    defaultMessage: 'Draws images through the generate_image tool.',
  },
  videoName: { id: 'serviceSettings.videoName', defaultMessage: 'Video' },
  videoDescription: {
    id: 'serviceSettings.videoDescription',
    defaultMessage: 'Saved for a later video service. Goose does not call it yet.',
  },
  verify: { id: 'serviceSettings.verify', defaultMessage: 'Verify' },
  verifying: { id: 'serviceSettings.verifying', defaultMessage: 'Verifying' },
  verifyOk: {
    id: 'serviceSettings.verifyOk',
    defaultMessage: 'Available · {latency} ms',
  },
  verifyFailed: {
    id: 'serviceSettings.verifyFailed',
    defaultMessage: 'Unavailable · {error}',
  },
  verifyPlay: { id: 'serviceSettings.verifyPlay', defaultMessage: 'Play' },
  verifyModelNotListed: {
    id: 'serviceSettings.verifyModelNotListed',
    defaultMessage: 'Endpoint reachable, but this model was not in the model list',
  },
  verifyReply: { id: 'serviceSettings.verifyReply', defaultMessage: 'Reply: {text}' },
  verifyTranscript: {
    id: 'serviceSettings.verifyTranscript',
    defaultMessage: 'Transcript: {text}',
  },
  verifyDimensions: {
    id: 'serviceSettings.verifyDimensions',
    defaultMessage: '{count} dimensions',
  },
  verifyAudioBytes: {
    id: 'serviceSettings.verifyAudioBytes',
    defaultMessage: '{count} bytes',
  },
  verifyUnknown: {
    id: 'serviceSettings.verifyUnknown',
    defaultMessage: 'Verification failed',
  },
});

type ServiceId = 'chat' | 'vision' | 'stt' | 'realtime' | 'tts' | 'embedding' | 'image' | 'video';

const SERVICES: Array<{
  id: ServiceId;
  wired: boolean;
  name: keyof typeof i18n;
  description: keyof typeof i18n;
}> = [
  { id: 'chat', wired: true, name: 'chatName', description: 'chatDescription' },
  { id: 'vision', wired: true, name: 'visionName', description: 'visionDescription' },
  { id: 'stt', wired: true, name: 'sttName', description: 'sttDescription' },
  { id: 'realtime', wired: false, name: 'realtimeName', description: 'realtimeDescription' },
  { id: 'tts', wired: true, name: 'ttsName', description: 'ttsDescription' },
  { id: 'embedding', wired: true, name: 'embeddingName', description: 'embeddingDescription' },
  { id: 'image', wired: true, name: 'imageName', description: 'imageDescription' },
  { id: 'video', wired: false, name: 'videoName', description: 'videoDescription' },
];

type Option = { value: string; label: string };
type Selection = { provider: string | null; model: string | null };

type VerifyState =
  | { status: 'idle' }
  | { status: 'running' }
  | {
      status: 'ok';
      latencyMs: number;
      detail: string | null;
      audio: ServiceVerifyAudio | null;
    }
  | { status: 'failed'; error: string };

const idleVerify: VerifyState = { status: 'idle' };

function emptyVerifies(): Record<ServiceId, VerifyState> {
  return Object.fromEntries(SERVICES.map((service) => [service.id, idleVerify])) as Record<
    ServiceId,
    VerifyState
  >;
}

function verifyExtra(intl: IntlShape, service: ServiceId, detail: string | null): string | null {
  if (!detail) return null;
  if (service === 'image') {
    return detail === 'model_not_listed' ? intl.formatMessage(i18n.verifyModelNotListed) : detail;
  }
  if (service === 'embedding') {
    return intl.formatMessage(i18n.verifyDimensions, { count: detail });
  }
  if (service === 'tts') {
    return intl.formatMessage(i18n.verifyAudioBytes, { count: detail });
  }
  if (service === 'stt') {
    return intl.formatMessage(i18n.verifyTranscript, { text: detail });
  }
  return intl.formatMessage(i18n.verifyReply, { text: detail });
}

function playAudio(audio: ServiceVerifyAudio) {
  const element = new window.Audio(`data:${audio.mimeType};base64,${audio.data}`);
  void element.play();
}

function VerifyResult({ service, state }: { service: ServiceId; state: VerifyState }) {
  const intl = useIntl();
  if (state.status === 'ok') {
    const extra = verifyExtra(intl, service, state.detail);
    const summary = intl.formatMessage(i18n.verifyOk, { latency: state.latencyMs });
    const text = extra ? `${summary} · ${extra}` : summary;
    const audio = state.audio;
    return (
      <p className="mt-1 flex min-w-0 items-center gap-1 text-xs text-green-600 dark:text-green-400">
        <Check className="h-3 w-3 shrink-0" />
        <span className="truncate" title={text}>
          {text}
        </span>
        {audio && (
          <button type="button" className="shrink-0 underline" onClick={() => playAudio(audio)}>
            {intl.formatMessage(i18n.verifyPlay)}
          </button>
        )}
      </p>
    );
  }
  if (state.status === 'failed') {
    const text = intl.formatMessage(i18n.verifyFailed, { error: state.error });
    return (
      <p className="mt-1 truncate text-xs text-red-600 dark:text-red-400" title={text}>
        {text}
      </p>
    );
  }
  return null;
}

function providerKey(id: ServiceId) {
  return `GOOSE_SERVICE_${id.toUpperCase()}_PROVIDER`;
}

function modelKey(id: ServiceId) {
  return `GOOSE_SERVICE_${id.toUpperCase()}_MODEL`;
}

function asString(value: unknown): string | null {
  return typeof value === 'string' && value.trim() ? value.trim() : null;
}

const modelSelectClassNames = {
  container: () => 'w-full cursor-pointer relative',
  indicatorSeparator: () => 'h-0',
  control: () =>
    'border border-border-primary rounded-md w-full px-4 py-2 text-sm text-text-secondary hover:cursor-pointer',
  menu: () =>
    'mt-1 bg-background-primary border border-border-primary rounded-md text-text-secondary shadow-lg z-[9999] absolute',
  menuList: () => 'max-h-60 overflow-y-auto py-1',
  option: ({ isFocused, isSelected }: { isFocused: boolean; isSelected: boolean }) =>
    `py-2 px-4 text-sm cursor-pointer ${
      isSelected
        ? 'bg-background-inverse text-text-inverse'
        : isFocused
          ? 'bg-background-secondary text-text-primary'
          : 'text-text-primary'
    }`,
};

export default function ServicesSection() {
  const intl = useIntl();
  const { read, upsert, remove } = useConfig();
  const { changeModel, refreshCurrentModelAndProvider } = useModelAndProvider();
  const [providers, setProviders] = useState<Option[]>([]);
  const [selections, setSelections] = useState<Record<ServiceId, Selection>>(
    () =>
      Object.fromEntries(
        SERVICES.map((service) => [service.id, { provider: null, model: null }])
      ) as Record<ServiceId, Selection>
  );
  const [modelsByProvider, setModelsByProvider] = useState<Record<string, Option[]>>({});
  const [verifies, setVerifies] = useState<Record<ServiceId, VerifyState>>(emptyVerifies);
  const verifyGeneration = useRef<Partial<Record<ServiceId, number>>>({});

  const loadModels = useCallback(async (providerId: string) => {
    if (!providerId) return;
    try {
      const models = await acpListSupportedModels(providerId);
      setModelsByProvider((current) => ({
        ...current,
        [providerId]: models.map((model) => ({ value: model, label: model })),
      }));
    } catch (error) {
      console.error(`Failed to list models for ${providerId}`, error);
      setModelsByProvider((current) => ({ ...current, [providerId]: [] }));
    }
  }, []);

  const load = useCallback(async () => {
    const details = await acpListProviderDetails();
    setProviders(
      details
        .filter((provider) => provider.is_configured)
        .map((provider) => ({
          value: provider.name,
          label: provider.metadata.display_name,
        }))
    );

    const defaults = await acpReadDefaults();
    const next = {} as Record<ServiceId, Selection>;
    for (const service of SERVICES) {
      if (service.id === 'chat') {
        next.chat = {
          provider: defaults.providerId ?? null,
          model: defaults.modelId ?? null,
        };
        continue;
      }
      next[service.id] = {
        provider: asString(await read(providerKey(service.id), false)),
        model: asString(await read(modelKey(service.id), false)),
      };
    }
    setSelections(next);
    const providerIds = new Set(
      Object.values(next)
        .map((selection) => selection.provider)
        .filter((provider): provider is string => Boolean(provider))
    );
    await Promise.all([...providerIds].map((providerId) => loadModels(providerId)));
  }, [loadModels, read]);

  useEffect(() => {
    load().catch((error) => console.error('Failed to load service settings', error));
  }, [load]);

  const clearVerify = (id: ServiceId) => {
    verifyGeneration.current[id] = (verifyGeneration.current[id] ?? 0) + 1;
    setVerifies((current) => ({ ...current, [id]: idleVerify }));
  };

  const verify = async (id: ServiceId) => {
    const selection = selections[id];
    if (!selection.provider || !selection.model) return;
    const generation = (verifyGeneration.current[id] ?? 0) + 1;
    verifyGeneration.current[id] = generation;
    setVerifies((current) => ({ ...current, [id]: { status: 'running' } }));
    try {
      const result = await acpVerifyService(id, selection.provider, selection.model);
      if (verifyGeneration.current[id] !== generation) return;
      setVerifies((current) => ({
        ...current,
        [id]: result.ok
          ? {
              status: 'ok',
              latencyMs: result.latencyMs,
              detail: result.detail,
              audio: result.audio,
            }
          : {
              status: 'failed',
              error: result.error ?? intl.formatMessage(i18n.verifyUnknown),
            },
      }));
    } catch (error) {
      if (verifyGeneration.current[id] !== generation) return;
      const message = error instanceof Error ? error.message : String(error);
      setVerifies((current) => ({ ...current, [id]: { status: 'failed', error: message } }));
    }
  };

  const saveService = async (id: ServiceId, selection: Selection) => {
    clearVerify(id);
    setSelections((current) => ({ ...current, [id]: selection }));
    if (id === 'chat') {
      if (selection.provider && selection.model) {
        const saved = await changeModel(null, {
          name: selection.model,
          provider: selection.provider,
        });
        if (!saved) {
          await load();
        }
        return;
      }
      if (!selection.provider && !selection.model) {
        await acpClearDefaults();
        await refreshCurrentModelAndProvider();
      }
      return;
    }

    if (!selection.provider) {
      await remove(providerKey(id), false);
      await remove(modelKey(id), false);
      return;
    }
    await upsert(providerKey(id), selection.provider, false);
    if (selection.model) {
      await upsert(modelKey(id), selection.model, false);
    } else {
      await remove(modelKey(id), false);
    }
  };

  const modelOptions = (selection: Selection): Option[] => {
    const listed = selection.provider ? (modelsByProvider[selection.provider] ?? []) : [];
    if (selection.model && !listed.some((option) => option.value === selection.model)) {
      return [{ value: selection.model, label: selection.model }, ...listed];
    }
    return listed;
  };

  return (
    <Card className="pb-2 rounded-lg">
      <CardHeader className="pb-0">
        <CardTitle>{intl.formatMessage(i18n.title)}</CardTitle>
        <CardDescription>{intl.formatMessage(i18n.description)}</CardDescription>
      </CardHeader>
      <CardContent className="px-4 space-y-3">
        {SERVICES.map((service) => {
          const selection = selections[service.id];
          const providerValue =
            providers.find((option) => option.value === selection.provider) ?? null;
          return (
            <div
              key={service.id}
              className="grid grid-cols-1 items-center gap-2 border-b border-border-primary py-3 last:border-b-0 md:grid-cols-[minmax(0,1.2fr)_minmax(0,1fr)_minmax(0,1fr)_auto]"
            >
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <h3 className="text-sm text-text-primary">
                    {intl.formatMessage(i18n[service.name])}
                  </h3>
                  {!service.wired && (
                    <span className="text-xs text-text-secondary">
                      {intl.formatMessage(i18n.notWired)}
                    </span>
                  )}
                </div>
                <p className="text-xs text-text-secondary">
                  {intl.formatMessage(i18n[service.description])}
                </p>
                <VerifyResult service={service.id} state={verifies[service.id]} />
              </div>
              <Select
                inputId={`${service.id}-provider`}
                aria-label={`${intl.formatMessage(i18n[service.name])} ${intl.formatMessage(i18n.providerPlaceholder)}`}
                options={providers}
                value={providerValue}
                placeholder={intl.formatMessage(i18n.providerPlaceholder)}
                isClearable
                onChange={(option) => {
                  const provider = (option as Option | null)?.value ?? null;
                  if (provider) {
                    loadModels(provider).catch((error) =>
                      console.error(`Failed to list models for ${provider}`, error)
                    );
                  }
                  saveService(service.id, { provider, model: null }).catch((error) =>
                    console.error(`Failed to save ${service.id} provider`, error)
                  );
                }}
              />
              <CreatableSelect
                unstyled
                inputId={`${service.id}-model`}
                aria-label={`${intl.formatMessage(i18n[service.name])} ${intl.formatMessage(i18n.modelPlaceholder)}`}
                classNames={modelSelectClassNames}
                options={modelOptions(selection)}
                value={selection.model ? { value: selection.model, label: selection.model } : null}
                placeholder={intl.formatMessage(i18n.modelPlaceholder)}
                isClearable
                isDisabled={!selection.provider}
                formatCreateLabel={(input) => input}
                onChange={(option) => {
                  saveService(service.id, {
                    provider: selection.provider,
                    model: option?.value ?? null,
                  }).catch((error) => console.error(`Failed to save ${service.id} model`, error));
                }}
              />
              {service.wired ? (
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  className="justify-self-start"
                  aria-label={`${intl.formatMessage(i18n[service.name])} ${intl.formatMessage(i18n.verify)}`}
                  disabled={
                    !selection.provider ||
                    !selection.model ||
                    verifies[service.id].status === 'running'
                  }
                  onClick={() => {
                    verify(service.id).catch((error) =>
                      console.error(`Failed to verify ${service.id}`, error)
                    );
                  }}
                >
                  {verifies[service.id].status === 'running' && (
                    <Loader2 className="h-3 w-3 animate-spin" />
                  )}
                  {intl.formatMessage(
                    verifies[service.id].status === 'running' ? i18n.verifying : i18n.verify
                  )}
                </Button>
              ) : (
                <div />
              )}
            </div>
          );
        })}
      </CardContent>
    </Card>
  );
}
