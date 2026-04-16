import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import { defineComponent, h } from 'vue'

describe('frontend smoke', () => {
  it('vitest can run', () => {
    expect(1 + 1).toBe(2)
  })

  it('jsdom environment is available', () => {
    expect(typeof window).toBe('object')
    expect(typeof document).toBe('object')
  })

  it('can mount a basic Vue component via @vue/test-utils', () => {
    const HelloWorld = defineComponent({
      name: 'HelloWorld',
      render() {
        return h('div', { class: 'hello' }, 'beanfun-next')
      },
    })

    const wrapper = mount(HelloWorld)

    expect(wrapper.text()).toBe('beanfun-next')
    expect(wrapper.find('.hello').exists()).toBe(true)
  })
})
