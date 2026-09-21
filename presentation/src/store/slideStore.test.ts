import { beforeEach, describe, expect, it } from 'vitest';
import type { SlidePayload } from '../lib/events';
import { useSlideStore } from './slideStore';

const samplePayload: SlidePayload = {
  index: 1,
  total: 5,
  title: 'Agenda',
  body: 'Topics for today',
  notes: '',
  background_path: null,
  layout_unsupported: false,
};

describe('useSlideStore', () => {
  beforeEach(() => {
    useSlideStore.setState({ current: null });
  });

  it('setSlide stores the payload as-is', () => {
    useSlideStore.getState().setSlide(samplePayload);
    expect(useSlideStore.getState().current).toEqual(samplePayload);
  });
});
