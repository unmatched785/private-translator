# Hy-MT2 local benchmark

Generated: 2026-07-24T12:37:03.885Z

## ko-en-01 · conversation · 492 ms

**Source (ko):** 그건 좀 아닌 것 같은데, 그렇다고 완전히 틀렸다는 뜻은 아니야.

**Translation (en):** That seems a bit off, but it doesn’t mean it’s completely wrong.

**Checks:** soft disagreement; does not overstate the second clause

## ko-en-02 · ellipsis · 388 ms

**Source (ko):** 어제 그 얘기 꺼냈다가 분위기만 싸해졌어.

**Translation (en):** Yesterday I brought that up, and it just made the atmosphere tense.

**Checks:** implicit subject; idiomatic social atmosphere

## ko-en-03 · business · 512 ms

**Source (ko):** 검토 결과, 이번 분기에는 출시를 미루는 편이 손실을 줄일 가능성이 높습니다.

**Translation (en):** After review, it is more likely that delaying the launch this quarter will help reduce losses.

**Checks:** formal register; probabilistic claim preserved

## ko-en-04 · technical · 748 ms

**Source (ko):** 요청 본문은 로그에 남기지 말고, 실패한 요청의 상태 코드와 처리 시간만 기록하세요.

**Translation (en):** The request body should not be recorded in the log, only the status code and processing time of failed requests should be recorded.

**Checks:** request body versus metadata; imperative preserved

## ko-en-05 · numbers · 886 ms

**Source (ko):** 할인율은 17.5%이고 최종 가격은 128,900원입니다. 자세한 내용은 https://example.com/sale에서 확인하세요.

**Translation (en):** The discount rate is 17.5%, and the final price is 128,900 won. For more details, please check https://example.com/sale.

**Checks:** 17.5%; 128,900; URL exact

## ko-en-06 · idiom · 510 ms

**Source (ko):** 싼 게 비지떡이라지만, 이번 제품은 가격에 비해 꽤 괜찮았다.

**Translation (en):** Though cheap things are hard to come by, this product was quite good for the price.

**Checks:** idiom conveyed naturally; contrast preserved

## ko-en-07 · honorific · 550 ms

**Source (ko):** 번거로우시겠지만 오늘 안으로 확인해 주시면 감사하겠습니다.

**Translation (en):** I would appreciate it if you could check it out by today, even though it’s troublesome.

**Checks:** polite request; deadline preserved

## ko-en-08 · ambiguity · 362 ms

**Source (ko):** 민수가 지훈에게 자기가 먼저 가겠다고 말했다.

**Translation (en):** Minsoo told Jihoon that he would go first.

**Checks:** does not invent which person '자기' refers to; reported speech

## ko-en-09 · product · 515 ms

**Source (ko):** 처음에는 빨라 보이지만 기록이 쌓일수록 검색 결과가 눈에 띄게 느려집니다.

**Translation (en):** At first, it seems fast, but as records accumulate, the search results become noticeably slower.

**Checks:** time progression; search results are subject of slowdown

## ko-en-10 · tone · 531 ms

**Source (ko):** 솔직히 기대를 많이 안 했는데, 생각보다 훨씬 잘 만들었더라.

**Translation (en):** Honestly, I wasn’t expecting too much, but it was made much better than I thought.

**Checks:** casual tone; pleasant surprise

## ko-en-11 · formatting · 505 ms

**Source (ko):** 필수 조건:  
- 계정 없음  
- 광고 없음  
- 번역문 저장 없음

**Translation (en):** Required conditions:    
- No account    
- No ads    
- No translation files saved

**Checks:** list structure; three negations preserved

## ko-en-12 · metaphor · 375 ms

**Source (ko):** 지금 기능을 더 붙이는 건 새는 배에 페인트칠하는 것과 다르지 않다.

**Translation (en):** Adding more features now is no different from painting a leaky ship.

**Checks:** metaphor preserved; critical tone

## en-ko-01 · conversation · 1220 ms

**Source (en):** I'm not sold on the idea yet, but I'm willing to hear you out.

**Translation (ko):** 아직 그 생각에 전적으로 동의하지는 않지만, 당신의 말을 듣고 싶습니다.

**Checks:** not literally about selling; open-minded contrast

## en-ko-02 · idiom · 963 ms

**Source (en):** The launch went smoothly, but the support team has been putting out fires ever since.

**Translation (ko):** 발사는 순조롭게 진행되었지만, 지원팀은 그 이후로 계속 문제를 해결하고 있습니다.

**Checks:** figurative firefighting; ongoing aftermath

## en-ko-03 · business · 938 ms

**Source (en):** Revenue grew by 8%, although the increase was largely offset by higher infrastructure costs.

**Translation (ko):** 수익은 8% 증가했지만, 이 증가분은 대부분 인프라 비용 상승으로 상쇄되었습니다.

**Checks:** 8%; offset relationship; formal register

## en-ko-04 · technical · 1145 ms

**Source (en):** Cache the model after the first download, but fail loudly if its checksum no longer matches.

**Translation (ko):** 첫 번째 다운로드 후 모델을 캐시하되, 체크섬프가 더 이상 일치하지 않으면 큰 소리로 실패합니다.

**Checks:** cache imperative; fail loudly meaning; checksum condition

## en-ko-05 · numbers · 1061 ms

**Source (en):** Upload limit: 25 MB. Requests time out after 30 seconds. See https://example.org/docs?v=2.

**Translation (ko):** 업로드 제한: 25 MB. 요청은 30초 후에 시간 초과됩니다. https://example.org/docs?v=2를 참조하세요.

**Checks:** 25 MB; 30 seconds; URL exact

## en-ko-06 · pragmatics · 641 ms

**Source (en):** Could you take another look when you get a chance? There's no rush.

**Translation (ko):** 기회가 되면 다시 한 번 살펴보세요. 서두를 필요는 없습니다.

**Checks:** polite request; no urgency

## en-ko-07 · ambiguity · 728 ms

**Source (en):** Alex told Jordan that their draft needed more work.

**Translation (ko):** 알렉스는 조던에게 그들의 초안에 더 많은 작업이 필요하다고 말했다.

**Checks:** does not invent gender; ownership ambiguity not falsely resolved

## en-ko-08 · tone · 675 ms

**Source (en):** It technically works, which is about the nicest thing I can say about it.

**Translation (ko):** 기술적으로는 작동합니다. 이것이 이 제품에 대해 할 수 있는 가장 긍정적인 말입니다.

**Checks:** dry criticism; not rendered as genuine praise

## en-ko-09 · product · 652 ms

**Source (en):** Users should be able to try the translator before they are asked to create an account.

**Translation (ko):** 사용자는 계정을 만들라는 요청 전에 번역기를 시도할 수 있어야 합니다.

**Checks:** sequence of actions; normative requirement

## en-ko-10 · phrasal · 984 ms

**Source (en):** We ruled out the quick fix because it would only move the problem somewhere else.

**Translation (ko):** 우리는 즉각적인 해결책을 제외했습니다. 왜냐하면 그건 문제를 다른 곳으로 옮길 뿐이기 때문입니다.

**Checks:** ruled out; problem displacement

## en-ko-11 · formatting · 1057 ms

**Source (en):** Before release:  
1. Verify the model hash.  
2. Disconnect the network.  
3. Translate the test sentence again.

**Translation (ko):** 출시 전:  
1. 모델 해시를 확인합니다.  
2. 네트워크를 끊습니다.  
3. 테스트 문장을 다시 번역합니다.

**Checks:** numbered structure; ordered sequence

## en-ko-12 · metaphor · 747 ms

**Source (en):** A polished interface cannot rescue a product that has no reason to exist.

**Translation (ko):** 매끄러운 인터페이스는 존재할 이유가 없는 제품을 구원할 수 없다.

**Checks:** rescue is metaphorical; strong critical tone

