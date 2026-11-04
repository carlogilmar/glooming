defmodule MyApp.Orders.Pipeline do
  @moduledoc """
  Consumes order events from SQS, enriches them, and writes them in bulk.

  ## Routing
  Paid orders go to the `:ledger` batcher, partitioned by account, so one
  account's postings stay in order. Everything else is stored by `:default`.
  """
  use Broadway

  require Logger

  alias Broadway.Message
  alias MyApp.Customers
  alias MyApp.Orders.{Decoder, Enricher, Ledger, Store}

  def start_link(_opts) do
    Broadway.start_link(__MODULE__,
      name: __MODULE__,
      producer: [
        module:
          {BroadwaySQS.Producer,
           queue_url: Application.fetch_env!(:my_app, :orders_queue_url),
           max_number_of_messages: 10,
           visibility_timeout: 300},
        concurrency: 2
      ],
      processors: [
        default: [
          concurrency: Application.get_env(:my_app, :order_processors, 50),
          max_demand: 10
        ]
      ],
      batchers: [
        default: [concurrency: 4, batch_size: 200, batch_timeout: 1_000],
        ledger: [concurrency: 1, batch_size: 50, batch_timeout: 5_000]
      ],
      context: %{
        source: :orders,
        region: Application.get_env(:my_app, :region, "us-east-1")
      }
    )
  end

  # One lookup for every customer in the batch, instead of one per message.
  @impl true
  def prepare_messages(messages, _context) do
    ids = Enum.map(messages, &Decoder.customer_id(&1.data))
    customers = Customers.fetch_many(ids)
    Enum.map(messages, &Message.put_data(&1, {&1.data, customers}))
  end

  @impl true
  def handle_message(_processor, message, _context) do
    with {:ok, order} <- Decoder.decode(message.data),
         {:ok, order} <- Enricher.add_totals(order),
         :ok <- validate(order) do
      message
      |> Message.put_data(order)
      |> route(order)
    else
      {:error, reason} -> Message.failed(message, reason)
    end
  end

  defp validate(%{total: total}) when total > 0, do: :ok
  defp validate(_order), do: {:error, :empty_order}

  defp route(message, %{status: :paid} = order) do
    message
    |> Message.put_batcher(:ledger)
    |> Message.put_batch_key(order.account_id)
  end

  defp route(message, _order), do: message

  @impl true
  def handle_batch(:default, messages, _batch_info, _context) do
    messages |> Enum.map(& &1.data) |> Store.insert_all()
    messages
  end

  def handle_batch(:ledger, messages, batch_info, _context) do
    case Ledger.post(batch_info.batch_key, Enum.map(messages, & &1.data)) do
      :ok -> messages
      {:error, reason} -> Enum.map(messages, &Message.failed(&1, reason))
    end
  end

  @impl true
  def handle_failed(messages, _context) do
    Enum.each(messages, &Logger.warning("order failed", reason: elem(&1.status, 1)))
    messages
  end
end
