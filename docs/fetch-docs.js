import { execSync } from 'child_process'

export const versions = ['rmk-v0.7.8']

versions.forEach((version) => {
  const command = `git archive heads/${version} docs/main | tar -x -C docs/ --transform "s|docs/main|${version}|"`
  console.log(`Fetched docs ${version}`)
  execSync(command, { stdio: 'inherit', shell: 'bash' })
})
