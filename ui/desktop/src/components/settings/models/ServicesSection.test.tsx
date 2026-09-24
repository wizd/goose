import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  acpClearDefaults,
  acpListProviderDetails,
  acpListSupportedModels,
  acpReadDefaults,
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
    expect(upsert).not.toHaveBeenCalledWith('GOOSE_SERVICE_CHAT_PROVIDER', expect.anything(), false);
  });
});
