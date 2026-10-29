export type ResultSuccess<T> = { success: true; data: T; message: string }

export type ResultError<E> = { success: false; error: E; message: string }

export type Result<T, E> = ResultSuccess<T> | ResultError<E>

export function succeed<T>(data: T, message: string): ResultSuccess<T> {
  return { success: true, data, message }
}

export function refuse<E>(error: E, message: string): ResultError<E> {
  return { success: false, error, message }
}
