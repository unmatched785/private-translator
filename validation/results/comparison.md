# Translation viability comparison

Generated: 2026-07-25T00:52:53.643Z

## Summary

| Engine | Semantic score | Major failures | Mean latency | p95 latency |
| --- | ---: | ---: | ---: | ---: |
| Hy-MT2 1.8B Q4 | 40/48 (83.3%) | 2 | 716 ms | 1145 ms |
| Chrome on-device Translator | 25/48 (52.1%) | 7 | 15 ms | 21 ms |

Scoring: 2 = usable; 1 = meaning mostly survives with a weakness; 0 = major semantic failure.

Limitation: Scores are a first-pass human semantic review by the project evaluator, not an independent blinded panel.

## Case results

### ko-en-01 · conversation

Source: 그건 좀 아닌 것 같은데, 그렇다고 완전히 틀렸다는 뜻은 아니야.

- Hy-MT2 (2/2, 492 ms): That seems a bit off, but it doesn’t mean it’s completely wrong.
  - Soft disagreement and qualification are both preserved.
- Chrome (0/2, 26 ms): I don't think that's a bit, but that doesn't mean I'm completely wrong.
  - Changes the speaker and says 'I'm completely wrong'.

### ko-en-02 · ellipsis

Source: 어제 그 얘기 꺼냈다가 분위기만 싸해졌어.

- Hy-MT2 (2/2, 388 ms): Yesterday I brought that up, and it just made the atmosphere tense.
  - The tense social atmosphere is conveyed naturally.
- Chrome (0/2, 10 ms): I brought up that story yesterday, but only the atmosphere became cheap.
  - Translates 싸해졌어 as the atmosphere becoming cheap.

### ko-en-03 · business

Source: 검토 결과, 이번 분기에는 출시를 미루는 편이 손실을 줄일 가능성이 높습니다.

- Hy-MT2 (2/2, 512 ms): After review, it is more likely that delaying the launch this quarter will help reduce losses.
  - Formal tone and probability are preserved.
- Chrome (1/2, 16 ms): As a result of the review, it is more likely that the side that delays launches will reduce losses this quarter.
  - Meaning mostly survives, but 'the side that delays launches' is unnatural and less precise.

### ko-en-04 · technical

Source: 요청 본문은 로그에 남기지 말고, 실패한 요청의 상태 코드와 처리 시간만 기록하세요.

- Hy-MT2 (2/2, 748 ms): The request body should not be recorded in the log, only the status code and processing time of failed requests should be recorded.
  - Correctly separates request-body logging from failure metadata.
- Chrome (2/2, 16 ms): Don't leave the request body in the log, just record the status code and processing time of the failed request.
  - Correctly separates request-body logging from failure metadata.

### ko-en-05 · numbers

Source: 할인율은 17.5%이고 최종 가격은 128,900원입니다. 자세한 내용은 https://example.com/sale에서 확인하세요.

- Hy-MT2 (2/2, 886 ms): The discount rate is 17.5%, and the final price is 128,900 won. For more details, please check https://example.com/sale.
  - Numbers, currency, and URL are preserved.
- Chrome (2/2, 20 ms): The discount rate is 17.5%, and the final price is 128,900 won. For more information, check https://example.com/sale.
  - Numbers, currency, and URL are preserved.

### ko-en-06 · idiom

Source: 싼 게 비지떡이라지만, 이번 제품은 가격에 비해 꽤 괜찮았다.

- Hy-MT2 (0/2, 510 ms): Though cheap things are hard to come by, this product was quite good for the price.
  - Fails to convey the 싼 게 비지떡 idiom and changes its meaning.
- Chrome (0/2, 11 ms): The cheap ones are expensive, but this product was pretty good for the price.
  - Renders the idiom as 'the cheap ones are expensive'.

### ko-en-07 · honorific

Source: 번거로우시겠지만 오늘 안으로 확인해 주시면 감사하겠습니다.

- Hy-MT2 (1/2, 550 ms): I would appreciate it if you could check it out by today, even though it’s troublesome.
  - The request and deadline survive, but 'even though it is troublesome' is awkward and shifts the politeness.
- Chrome (2/2, 12 ms): It may be a hassle, but I would appreciate it if you check it out today.
  - The polite request and deadline are conveyed naturally.

### ko-en-08 · ambiguity

Source: 민수가 지훈에게 자기가 먼저 가겠다고 말했다.

- Hy-MT2 (2/2, 362 ms): Minsoo told Jihoon that he would go first.
  - The English pronoun leaves the intended person unresolved.
- Chrome (2/2, 8 ms): Minsu told Jihoon that he would go first.
  - The English pronoun leaves the intended person unresolved.

### ko-en-09 · product

Source: 처음에는 빨라 보이지만 기록이 쌓일수록 검색 결과가 눈에 띄게 느려집니다.

- Hy-MT2 (2/2, 515 ms): At first, it seems fast, but as records accumulate, the search results become noticeably slower.
  - The accumulation of records and search slowdown are preserved.
- Chrome (1/2, 13 ms): It looks fast at first, but the more you record, the slower your search results are.
  - Changes accumulated records into the user's act of recording.

### ko-en-10 · tone

Source: 솔직히 기대를 많이 안 했는데, 생각보다 훨씬 잘 만들었더라.

- Hy-MT2 (1/2, 531 ms): Honestly, I wasn’t expecting too much, but it was made much better than I thought.
  - The surprise survives, but 'it was made much better' is unnatural.
- Chrome (2/2, 14 ms): To be honest, I didn't expect much, but it was much better than I expected.
  - The casual surprise is conveyed cleanly.

### ko-en-11 · formatting

Source: 필수 조건:  
- 계정 없음  
- 광고 없음  
- 번역문 저장 없음

- Hy-MT2 (1/2, 505 ms): Required conditions:    
- No account    
- No ads    
- No translation files saved
  - List formatting survives, but 번역문 becomes 'translation files'.
- Chrome (1/2, 11 ms): Required conditions: - No account - No ads - No translations saved
  - The three conditions survive, but list formatting is flattened.

### ko-en-12 · metaphor

Source: 지금 기능을 더 붙이는 건 새는 배에 페인트칠하는 것과 다르지 않다.

- Hy-MT2 (2/2, 375 ms): Adding more features now is no different from painting a leaky ship.
  - The leaky-ship metaphor and criticism are preserved.
- Chrome (0/2, 12 ms): Adding more features now is no different from painting a bird's stomach.
  - Misreads 새는 배 as a bird's stomach.

### en-ko-01 · conversation

Source: I'm not sold on the idea yet, but I'm willing to hear you out.

- Hy-MT2 (2/2, 1220 ms): 아직 그 생각에 전적으로 동의하지는 않지만, 당신의 말을 듣고 싶습니다.
  - Not being convinced yet and willingness to listen are preserved.
- Chrome (0/2, 20 ms): 나는 아직 아이디어에 매진되지 않았지만 당신의 말을 듣고 싶습니다.
  - Treats 'sold on' as being sold out or devoted.

### en-ko-02 · idiom

Source: The launch went smoothly, but the support team has been putting out fires ever since.

- Hy-MT2 (2/2, 963 ms): 발사는 순조롭게 진행되었지만, 지원팀은 그 이후로 계속 문제를 해결하고 있습니다.
  - Correctly converts figurative firefighting into ongoing problem solving.
- Chrome (0/2, 15 ms): 발사는 순조롭게 진행되었지만 지원팀은 그 이후로 화재를 진압하고 있습니다.
  - Translates the figurative fires as literal fires.

### en-ko-03 · business

Source: Revenue grew by 8%, although the increase was largely offset by higher infrastructure costs.

- Hy-MT2 (2/2, 938 ms): 수익은 8% 증가했지만, 이 증가분은 대부분 인프라 비용 상승으로 상쇄되었습니다.
  - The 8% increase and offset relationship are clear.
- Chrome (2/2, 14 ms): 매출 증가는 인프라 비용 증가로 크게 상쇄되었지만 매출은 8% 증가했습니다.
  - The 8% increase and offset relationship are clear.

### en-ko-04 · technical

Source: Cache the model after the first download, but fail loudly if its checksum no longer matches.

- Hy-MT2 (0/2, 1145 ms): 첫 번째 다운로드 후 모델을 캐시하되, 체크섬프가 더 이상 일치하지 않으면 큰 소리로 실패합니다.
  - Contains a checksum typo and literally translates 'fail loudly' as making a loud sound.
- Chrome (0/2, 15 ms): 첫 번째 다운로드 후 모델을 캐시하지만 체크섬이 더 이상 일치하지 않으면 크게 실패합니다.
  - Literally renders 'fail loudly' as failing loudly or greatly.

### en-ko-05 · numbers

Source: Upload limit: 25 MB. Requests time out after 30 seconds. See https://example.org/docs?v=2.

- Hy-MT2 (2/2, 1061 ms): 업로드 제한: 25 MB. 요청은 30초 후에 시간 초과됩니다. https://example.org/docs?v=2를 참조하세요.
  - Limits, timeout, and URL are all preserved.
- Chrome (1/2, 21 ms): 업로드 제한: 25MB. 30초 후 시간 초과 요청. https://example.org/docs?v=2를 참조하십시오.
  - Details survive, but the timeout sentence becomes an awkward noun phrase.

### en-ko-06 · pragmatics

Source: Could you take another look when you get a chance? There's no rush.

- Hy-MT2 (2/2, 641 ms): 기회가 되면 다시 한 번 살펴보세요. 서두를 필요는 없습니다.
  - The polite request and lack of urgency are natural.
- Chrome (1/2, 16 ms): 기회가 되면 또 한 번 더 살펴볼 수 있나요? 서두르지 않습니다.
  - Meaning survives, but the wording is noticeably unnatural.

### en-ko-07 · ambiguity

Source: Alex told Jordan that their draft needed more work.

- Hy-MT2 (1/2, 728 ms): 알렉스는 조던에게 그들의 초안에 더 많은 작업이 필요하다고 말했다.
  - No gender is invented, but 'their draft' is rendered mechanically.
- Chrome (1/2, 13 ms): Alex는 Jordan에게 그들의 드래프트가 더 많은 작업이 필요하다고 말했습니다.
  - No gender is invented, but draft is transliterated and ownership remains awkward.

### en-ko-08 · tone

Source: It technically works, which is about the nicest thing I can say about it.

- Hy-MT2 (2/2, 675 ms): 기술적으로는 작동합니다. 이것이 이 제품에 대해 할 수 있는 가장 긍정적인 말입니다.
  - The dry criticism is conveyed as the most positive thing available to say.
- Chrome (1/2, 16 ms): 그것은 기술적으로 작동하며, 내가 그것에 대해 말할 수 있는 가장 좋은 것입니다.
  - The literal wording weakens the dry, dismissive tone.

### en-ko-09 · product

Source: Users should be able to try the translator before they are asked to create an account.

- Hy-MT2 (2/2, 652 ms): 사용자는 계정을 만들라는 요청 전에 번역기를 시도할 수 있어야 합니다.
  - The required order between trying and account creation is preserved.
- Chrome (2/2, 13 ms): 사용자는 계정을 만들라는 요청을 받기 전에 번역기를 사용해 볼 수 있어야 합니다.
  - The required order between trying and account creation is preserved.

### en-ko-10 · phrasal

Source: We ruled out the quick fix because it would only move the problem somewhere else.

- Hy-MT2 (2/2, 984 ms): 우리는 즉각적인 해결책을 제외했습니다. 왜냐하면 그건 문제를 다른 곳으로 옮길 뿐이기 때문입니다.
  - Ruling out the fix and merely moving the problem are preserved.
- Chrome (2/2, 11 ms): 문제를 다른 곳으로 옮기기 때문에 빠른 수정을 배제했습니다.
  - Ruling out the fix and merely moving the problem are preserved.

### en-ko-11 · formatting

Source: Before release:  
1. Verify the model hash.  
2. Disconnect the network.  
3. Translate the test sentence again.

- Hy-MT2 (2/2, 1057 ms): 출시 전:  
1. 모델 해시를 확인합니다.  
2. 네트워크를 끊습니다.  
3. 테스트 문장을 다시 번역합니다.
  - Numbered structure and sequence are preserved.
- Chrome (1/2, 17 ms): 출시 전: 1. 모델 해시를 확인합니다. 2. 네트워크 연결을 끊습니다. 3. 테스트 문장을 다시 번역합니다.
  - All steps survive, but the numbered layout is flattened.

### en-ko-12 · metaphor

Source: A polished interface cannot rescue a product that has no reason to exist.

- Hy-MT2 (2/2, 747 ms): 매끄러운 인터페이스는 존재할 이유가 없는 제품을 구원할 수 없다.
  - The metaphorical criticism is strong and natural enough.
- Chrome (1/2, 15 ms): 세련된 인터페이스는 존재할 이유가 없는 제품을 구출할 수 없습니다.
  - Meaning survives, but 'rescue' is rendered too literally.

