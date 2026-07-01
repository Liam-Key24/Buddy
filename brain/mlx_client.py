from typing import Iterable

from openai import OpenAI


class MLXClient:
    def __init__(self, base_url: str, model: str):
        self.client = OpenAI(base_url=base_url, api_key="not-needed")
        self.model = model

    def complete(
        self,
        system: str,
        messages: list[dict],
        max_tokens: int = 512,
        temperature: float = 0.7,
    ) -> str:
        full_messages = [{"role": "system", "content": system}, *messages]
        response = self.client.chat.completions.create(
            model=self.model,
            messages=full_messages,
            max_tokens=max_tokens,
            temperature=temperature,
        )
        return response.choices[0].message.content or ""

    def stream(
        self,
        system: str,
        messages: list[dict],
        max_tokens: int = 512,
        temperature: float = 0.7,
    ) -> Iterable[str]:
        full_messages = [{"role": "system", "content": system}, *messages]
        stream = self.client.chat.completions.create(
            model=self.model,
            messages=full_messages,
            max_tokens=max_tokens,
            temperature=temperature,
            stream=True,
        )
        for chunk in stream:
            delta = chunk.choices[0].delta.content
            if delta:
                yield delta
