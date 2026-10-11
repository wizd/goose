import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  acpClearDefaults,
  acpListProviderDetails,
  acpListSupportedModels,
  acpReadDefaults,
  acpVerifyService,
} from '../../../acp/providers';
import { IntlTestWrapper } from '../../../i18n/test-utils';
import { useConfig } from '../../ConfigContext';
import { useModelAndProvider } from '../../ModelAndProviderContext';
import ServicesSection from './ServicesSection';

vi.mock('../../../acp/providers', () => ({
  acpListProviderDetails: vi.fn(),
  acpListSupportedModels: vi.fn(),
  acpReadDefaults: vi.fn(),
  acpClearDefaults: vi.fn(),
  acpVerifyService: vi.fn(),
}));

vi.mock('../../ConfigContext', () => ({ useConfig: vi.fn() }));
vi.mock('../../ModelAndProviderContext', () => ({ useModelAndProvider: vi.fn() }));

const providerDetails = [
  {
    name: 'vcorp',
    is_configured: true,
    metadata: { display_name: 'VCorp' },
  },
  {
    name: 'openai',
    is_configured: false,
    metadata: { display_name: 'OpenAI' },
  },
];

describe('ServicesSection', () => {
  const read = vi.fn();
  const upsert = vi.fn();
  const remove = vi.fn();
  const changeModel = vi.fn();
  const refreshCurrentModelAndProvider = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    read.mockResolvedValue(null);
    upsert.mockResolvedValue(undefined);
    remove.mockResolvedValue(undefined);
    changeModel.mockResolvedValue(true);
    refreshCurrentModelAndProvider.mockResolvedValue(undefined);
    vi.mocked(useConfig).mockReturnValue({ read, upsert, remove } as unknown as ReturnType<
      typeof useConfig
    >);
    vi.mocked(useModelAndProvider).mockReturnValue({
      changeModel,
      refreshCurrentModelAndProvider,
    } as unknown as ReturnType<typeof useModelAndProvider>);
    vi.mocked(acpListProviderDetails).mockResolvedValue(providerDetails as never);
    vi.mocked(acpReadDefaults).mockResolvedValue({
      providerId: 'vcorp',
      modelId: 'google/gemini-3.8-flash',
    });
    vi.mocked(acpListSupportedModels).mockResolvedValue([
      'google/gemini-3.8-flash',
      'google/gemini-pro',
    ]);
    vi.mocked(acpClearDefaults).mockResolvedValue(undefined);
  });

  it('loads configured providers and marks unwired services', async () => {
    render(<ServicesSection />, { wrapper: IntlTestWrapper });

    expect(await screen.findByText('Vision')).toBeInTheDocument();
    expect(screen.getAllByText('Not connected yet').length).toBeGreaterThan(0);
    expect(acpListProviderDetails).toHaveBeenCalled();
    await waitFor(() => expect(acpReadDefaults).toHaveBeenCalled());
    expect(screen.queryByText('OpenAI')).not.toBeInTheDocument();
  });

  it('saves a vision provider and model', async () => {
    const user = userEvent.setup();
    render(<ServicesSection />, { wrapper: IntlTestWrapper });
    await screen.findByText('Vision');

    await user.click(screen.getByLabelText('Vision Provider'));
    await user.click(await screen.findByRole('option', { name: 'VCorp' }));

    await waitFor(() =>
      expect(upsert).toHaveBeenCalledWith('GOOSE_SERVICE_VISION_PROVIDER', 'vcorp', false)
    );

    await user.click(screen.getByLabelText('Vision Model'));
    fireEvent.click(await screen.findByRole('option', { name: 'google/gemini-3.8-flash' }));

    await waitFor(() =>
      expect(upsert).toHaveBeenCalledWith(
        'GOOSE_SERVICE_VISION_MODEL',
        'google/gemini-3.8-flash',
        false
      )
    );
  });

  it('writes chat through the existing defaults', async () => {
    const user = userEvent.setup();
    render(<ServicesSection />, { wrapper: IntlTestWrapper });
    await screen.findByText('Chat');
    await waitFor(() => expect(acpListSupportedModels).toHaveBeenCalledWith('vcorp'));

    await user.click(screen.getByLabelText('Chat Model'));
    await user.click(await screen.findByRole('option', { name: 'google/gemini-pro' }));

    await waitFor(() =>
      expect(changeModel).toHaveBeenCalledWith(null, {
        name: 'google/gemini-pro',
        provider: 'vcorp',
      })
    );
    expect(upsert).not.toHaveBeenCalledWith(
      'GOOSE_SERVICE_CHAT_PROVIDER',
      expect.anything(),
      false
    );
  });

  it('verifies the selected chat model and clears the result when the model changes', async () => {
    const user = userEvent.setup();
    vi.mocked(acpVerifyService).mockResolvedValue({
      ok: true,
      latencyMs: 820,
      detail: 'OK',
      error: null,
      audio: null,
    });
    render(<ServicesSection />, { wrapper: IntlTestWrapper });
    await screen.findByText('Chat');
    await waitFor(() => expect(acpListSupportedModels).toHaveBeenCalledWith('vcorp'));

    expect(screen.getByRole('button', { name: 'Vision Verify' })).toBeDisabled();
    await user.click(screen.getByRole('button', { name: 'Chat Verify' }));

    expect(await screen.findByText('Available · 820 ms · Reply: OK')).toBeInTheDocument();
    expect(acpVerifyService).toHaveBeenCalledWith('chat', 'vcorp', 'google/gemini-3.8-flash');

    await user.click(screen.getByLabelText('Chat Model'));
    await user.click(await screen.findByRole('option', { name: 'google/gemini-pro' }));

    await waitFor(() =>
      expect(screen.queryByText('Available · 820 ms · Reply: OK')).not.toBeInTheDocument()
    );
  });

  it('shows a verification failure', async () => {
    const user = userEvent.setup();
    vi.mocked(acpVerifyService).mockResolvedValue({
      ok: false,
      latencyMs: 40,
      detail: null,
      error: 'bad key',
      audio: null,
    });
    render(<ServicesSection />, { wrapper: IntlTestWrapper });
    await screen.findByText('Chat');

    await user.click(screen.getByRole('button', { name: 'Chat Verify' }));

    expect(await screen.findByText('Unavailable · bad key')).toBeInTheDocument();
  });

  it('plays speech audio returned by verification', async () => {
    const user = userEvent.setup();
    const play = vi.fn().mockResolvedValue(undefined);
    const calls: string[] = [];
    class FakeAudio {
      constructor(url: string) {
        calls.push(url);
      }
      play = play;
    }
    vi.stubGlobal('Audio', FakeAudio);
    try {
      vi.mocked(acpVerifyService).mockResolvedValue({
        ok: true,
        latencyMs: 100,
        detail: '12',
        error: null,
        audio: { data: 'QQ==', mimeType: 'audio/mpeg' },
      });
      render(<ServicesSection />, { wrapper: IntlTestWrapper });
      await screen.findByText('Text to speech');

      await user.click(screen.getByLabelText('Text to speech Provider'));
      await user.click(await screen.findByRole('option', { name: 'VCorp' }));
      await user.click(screen.getByLabelText('Text to speech Model'));
      fireEvent.click(await screen.findByRole('option', { name: 'google/gemini-3.8-flash' }));
      await user.click(screen.getByRole('button', { name: 'Text to speech Verify' }));
      await user.click(await screen.findByRole('button', { name: 'Play' }));

      expect(acpVerifyService).toHaveBeenCalledWith('tts', 'vcorp', 'google/gemini-3.8-flash');
      expect(calls).toEqual(['data:audio/mpeg;base64,QQ==']);
      expect(play).toHaveBeenCalled();
      expect(screen.getByText(/12 bytes/)).toBeInTheDocument();
    } finally {
      vi.unstubAllGlobals();
    }
  });
});
