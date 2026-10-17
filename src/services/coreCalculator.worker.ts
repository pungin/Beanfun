import { findPerfectCores, formatPerfectCoreResult } from './coreCalculator'
import type { CoreCalculationRequest } from './coreCalculatorTask'

self.onmessage = ({ data }: MessageEvent<CoreCalculationRequest>): void => {
  const groups = findPerfectCores(data.items, data.skills)
  self.postMessage(
    formatPerfectCoreResult(groups, {
      mainLabel: data.labels.mainLabel,
      notFound: data.labels.notFound,
      formatGroup: (index) => data.labels.groupTemplate.split('{index}').join(String(index)),
    }),
  )
}
